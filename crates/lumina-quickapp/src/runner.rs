//! Runs a recipe's steps with Lumina's PHP, Composer, Node.js and database
//! client, streaming their output.
//!
//! Every command runs in its own kill-on-close process group (a Job Object on
//! Windows), so cancelling kills the whole tree — Composer, the PHP processes
//! it starts, npm and its node children — and nothing a command left behind
//! survives its step either.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::Ordering;
use std::time::Duration;

use lumina_packages::Cancel;
use lumina_platform::long_path::long;
use lumina_platform::process_group::{self, ProcessGroup, SpawnedChild};
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::files;
use crate::recipe::{Action, Recipe, Step, Vars, applies, is_relative_plain};
use crate::{Error, Result};

/// A program a step can run: Lumina's own, with leading arguments (PHP's
/// `-c php.ini`, `composer.phar` for Composer, server options for mysql).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tool {
    pub program: PathBuf,
    pub args: Vec<String>,
}

/// Everything a run needs from the app.
pub struct Context {
    /// The project folder; must not exist when the run starts.
    pub dir: PathBuf,
    /// The projects folder (`www`).
    pub www: PathBuf,
    /// Placeholder values (`name`, `dir`, `domain`, `url`, `db`, ...).
    pub values: BTreeMap<String, String>,
    /// By [`crate::recipe::PROGRAMS`] name.
    pub tools: BTreeMap<String, Tool>,
    /// Environment for every command (PATH with Lumina's tools first, PHPRC, ...).
    pub env: Vec<(String, String)>,
    /// `resources/quickapp/templates`.
    pub templates: PathBuf,
    /// Downloads go here; removed by the caller afterwards.
    pub work_dir: PathBuf,
    /// `http://host:port`, empty = the system's.
    pub proxy: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Step `index` of `total` (1-based) started.
    Step {
        index: usize,
        total: usize,
        label: String,
    },
    /// A line of output (or a note from the runner).
    Line {
        text: String,
    },
    Download {
        received: u64,
        total: Option<u64>,
    },
}

/// What happened, including what to undo after a failure.
#[derive(Debug)]
pub struct Report {
    pub result: Result<()>,
    /// Created by this run (it did not exist before).
    pub created_database: Option<String>,
}

type Emit<'a> = &'a (dyn Fn(Event) + Send + Sync);

struct Run<'a> {
    recipe: &'a Recipe,
    options: &'a BTreeMap<String, String>,
    ctx: &'a Context,
    cancel: &'a Cancel,
    emit: Emit<'a>,
    created_database: Option<String>,
}

/// Runs `recipe` with the chosen `options` (see [`Recipe::resolve_options`]).
pub async fn run(
    recipe: &Recipe,
    options: &BTreeMap<String, String>,
    ctx: &Context,
    cancel: &Cancel,
    emit: Emit<'_>,
) -> Report {
    let mut run = Run {
        recipe,
        options,
        ctx,
        cancel,
        emit,
        created_database: None,
    };
    let result = run.all().await;
    Report {
        result,
        created_database: run.created_database,
    }
}

impl Run<'_> {
    fn vars(&self) -> Vars<'_> {
        Vars {
            values: &self.ctx.values,
            recipe: self.recipe,
            options: self.options,
        }
    }

    fn note(&self, text: impl Into<String>) {
        (self.emit)(Event::Line { text: text.into() });
    }

    fn check_cancel(&self) -> Result<()> {
        if self.cancel.load(Ordering::Relaxed) {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    }

    async fn all(&mut self) -> Result<()> {
        if self.ctx.dir.exists() {
            return Err(Error::Other(format!(
                "{} already exists",
                self.ctx.dir.display()
            )));
        }
        std::fs::create_dir_all(&self.ctx.work_dir).map_err(|source| Error::Io {
            path: self.ctx.work_dir.clone(),
            source,
        })?;
        let steps: Vec<&Step> = self
            .recipe
            .steps
            .iter()
            .filter(|s| applies(s, self.options))
            .collect();
        let total = steps.len();
        for (i, step) in steps.into_iter().enumerate() {
            self.check_cancel()?;
            (self.emit)(Event::Step {
                index: i + 1,
                total,
                label: step.label.clone(),
            });
            match self.action(&step.action).await {
                Ok(()) => {}
                Err(Error::Cancelled) => return Err(Error::Cancelled),
                Err(err) if step.optional => {
                    self.note(format!("{}: {err} (continuing)", step.label));
                }
                Err(err) if !step.fallback.is_empty() => {
                    self.note(format!("{}: {err}", step.label));
                    self.note("Trying the alternative...".to_owned());
                    self.remove_project_dir().await?;
                    for fallback in step.fallback.iter().filter(|s| applies(s, self.options)) {
                        self.check_cancel()?;
                        (self.emit)(Event::Step {
                            index: i + 1,
                            total,
                            label: fallback.label.clone(),
                        });
                        Box::pin(self.action(&fallback.action)).await?;
                    }
                }
                Err(err) => return Err(err),
            }
        }
        Ok(())
    }

    /// The project folder did not exist when the run started, so anything in
    /// it came from this run.
    async fn remove_project_dir(&self) -> Result<()> {
        let dir = self.ctx.dir.clone();
        if !dir.exists() {
            return Ok(());
        }
        tokio::task::spawn_blocking(move || {
            std::fs::remove_dir_all(long(&dir)).map_err(|source| Error::Io { path: dir, source })
        })
        .await
        .map_err(|e| Error::Other(e.to_string()))?
    }

    /// A path inside the project folder (`.` is the folder itself).
    fn project_path(&self, rel: &str) -> Result<PathBuf> {
        let rel = self.vars().expand(rel)?;
        if rel == "." {
            return Ok(self.ctx.dir.clone());
        }
        if !is_relative_plain(&rel) {
            return Err(Error::Recipe(format!(
                "`{rel}` must be a path inside the project"
            )));
        }
        Ok(self.ctx.dir.join(rel))
    }

    fn work_path(&self, name: &str) -> Result<PathBuf> {
        let name = self.vars().expand(name)?;
        if !is_relative_plain(&name) || name.contains(['/', '\\']) {
            return Err(Error::Recipe(format!("bad file name `{name}`")));
        }
        Ok(self.ctx.work_dir.join(name))
    }

    fn tool(&self, program: &str) -> Result<&Tool> {
        self.ctx
            .tools
            .get(program)
            .ok_or_else(|| Error::Other(format!("{program} is not installed; add it in Quick Add")))
    }

    fn env(&self, step_env: &BTreeMap<String, String>) -> Result<Vec<(String, String)>> {
        let mut env = self.ctx.env.clone();
        for (k, v) in self.recipe.env.iter().chain(step_env) {
            env.push((k.clone(), self.vars().expand(v)?));
        }
        Ok(env)
    }

    async fn action(&mut self, action: &Action) -> Result<()> {
        match action {
            Action::Run {
                program,
                args,
                cwd,
                env,
            } => {
                let tool = self.tool(program)?.clone();
                let mut expanded = Vec::new();
                for arg in args {
                    let arg = self.vars().expand(arg)?;
                    if !arg.is_empty() {
                        expanded.push(arg);
                    }
                }
                let cwd = match cwd {
                    Some(cwd) => PathBuf::from(self.vars().expand(cwd)?),
                    None if self.ctx.dir.is_dir() => self.ctx.dir.clone(),
                    None => self.ctx.www.clone(),
                };
                let env = self.env(env)?;
                self.note(format!("> {program} {}", expanded.join(" ")));
                run_command(&tool, &expanded, &cwd, &env, self.cancel, self.emit, false)
                    .await
                    .map(|_| ())
            }
            Action::Download {
                url,
                github,
                asset,
                save_as,
            } => {
                let dest = self.work_path(save_as)?;
                let url = match (url, github, asset) {
                    (Some(url), _, _) => self.vars().expand(url)?,
                    (None, Some(repo), Some(asset)) => {
                        github_asset(repo, &self.vars().expand(asset)?, &self.ctx.proxy).await?
                    }
                    _ => return Err(Error::Recipe("download needs a source".into())),
                };
                self.note(format!("Downloading {url}"));
                // A leftover from an earlier run may be another version: start fresh.
                let _ = std::fs::remove_file(lumina_packages::download::part_path(&dest));
                let _ = std::fs::remove_file(&dest);
                let downloader = lumina_packages::download::Downloader::new(Some(&self.ctx.proxy))
                    .map_err(|e| Error::Download(e.to_string()))?;
                let emit = self.emit;
                downloader
                    .fetch(&[url.as_str()], &dest, self.cancel, &|received, total| {
                        emit(Event::Download { received, total })
                    })
                    .await
                    .map_err(|e| match e {
                        lumina_packages::Error::Cancelled => Error::Cancelled,
                        e => Error::Download(e.to_string()),
                    })?;
                Ok(())
            }
            Action::Extract { from, to, strip } => {
                let archive = self.work_path(from)?;
                let dest = self.project_path(to)?;
                let kind = if archive.to_string_lossy().ends_with(".zip") {
                    lumina_packages::Archive::Zip
                } else {
                    lumina_packages::Archive::TarGz
                };
                let release = lumina_packages::Release {
                    version: String::new(),
                    os: "any".into(),
                    arch: "any".into(),
                    url: String::new(),
                    mirrors: Vec::new(),
                    sha256: String::new(),
                    size: None,
                    archive: kind,
                    strip: *strip,
                    subdir: None,
                    file_name: None,
                    channel: None,
                    requires: Default::default(),
                    checksum_source: None,
                };
                self.note(format!("Extracting into {}", dest.display()));
                tokio::task::spawn_blocking(move || {
                    if let Some(parent) = dest.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    lumina_packages::extract::extract(&archive, &release, &dest)
                })
                .await
                .map_err(|e| Error::Other(e.to_string()))?
                .map_err(|e| Error::Other(e.to_string()))
            }
            Action::CreateDatabase { name } => {
                let db = self.vars().expand(name)?;
                if !files::is_safe_database(&db) {
                    return Err(Error::Database(format!(
                        "`{db}` is not a valid database name"
                    )));
                }
                let tool = self.tool("mysql")?.clone();
                let env = self.env(&BTreeMap::new())?;
                let cwd = self.ctx.www.clone();
                let (cancel, emit) = (self.cancel, self.emit);
                let query = |sql: String| {
                    let args = ["-N".to_owned(), "-B".to_owned(), "-e".to_owned(), sql];
                    let (tool, env, cwd) = (tool.clone(), env.clone(), cwd.clone());
                    async move { run_command(&tool, &args, &cwd, &env, cancel, emit, true).await }
                };
                let row = query(files::sql_inspect(&db)).await.map_err(db_error)?;
                let counts: Vec<u64> = row
                    .split_whitespace()
                    .filter_map(|n| n.parse().ok())
                    .collect();
                match counts.as_slice() {
                    [0, _] => {
                        query(files::sql_create(&db)).await.map_err(db_error)?;
                        self.created_database = Some(db.clone());
                        self.note(format!("Created database `{db}`"));
                    }
                    [_, 0] => self.note(format!("Using the existing empty database `{db}`")),
                    [_, tables] => {
                        return Err(Error::Database(format!(
                            "the database `{db}` already exists and has {tables} tables; choose another name or remove it"
                        )));
                    }
                    _ => {
                        return Err(Error::Database(format!(
                            "unexpected answer from the database server: {row:?}"
                        )));
                    }
                }
                Ok(())
            }
            Action::PatchEnv {
                file,
                copy_from,
                set,
            } => {
                let path = self.project_path(file)?;
                let text = match std::fs::read_to_string(&path) {
                    Ok(text) => text,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => match copy_from {
                        Some(from) => {
                            let from = self.project_path(from)?;
                            std::fs::read_to_string(&from)
                                .map_err(|source| Error::Io { path: from, source })?
                        }
                        None => String::new(),
                    },
                    Err(source) => return Err(Error::Io { path, source }),
                };
                let mut values = BTreeMap::new();
                for (k, v) in set {
                    values.insert(k.clone(), self.vars().expand(v)?);
                }
                write_file(&path, files::patch_env(&text, &values).as_bytes())?;
                self.note(format!("Updated {}", path.display()));
                Ok(())
            }
            Action::TemplateFile { template, to } => {
                let source = self.ctx.templates.join(template);
                let text = std::fs::read_to_string(&source).map_err(|source_err| Error::Io {
                    path: source.clone(),
                    source: source_err,
                })?;
                let dest = self.project_path(to)?;
                if dest.exists() {
                    return Err(Error::Other(format!("{} already exists", dest.display())));
                }
                write_file(&dest, self.vars().expand(&text)?.as_bytes())?;
                self.note(format!("Wrote {}", dest.display()));
                Ok(())
            }
        }
    }
}

fn db_error(e: Error) -> Error {
    match e {
        Error::Cancelled => Error::Cancelled,
        e => Error::Database(e.to_string()),
    }
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| Error::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    std::fs::write(path, bytes).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// The download URL of the newest release asset of `repo` named like `pattern`.
async fn github_asset(repo: &str, pattern: &str, proxy: &str) -> Result<String> {
    let mut builder = reqwest::Client::builder()
        .user_agent(concat!("Lumina/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(60));
    if !proxy.trim().is_empty() {
        builder = builder
            .proxy(reqwest::Proxy::all(proxy.trim()).map_err(|e| Error::Download(e.to_string()))?);
    }
    let client = builder
        .build()
        .map_err(|e| Error::Download(e.to_string()))?;
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let release: serde_json::Value = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|e| Error::Download(format!("{url}: {e}")))?
        .json()
        .await
        .map_err(|e| Error::Download(format!("{url}: {e}")))?;
    release["assets"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|a| {
            a["name"]
                .as_str()
                .is_some_and(|n| files::glob_match(pattern, n))
        })
        .and_then(|a| a["browser_download_url"].as_str())
        .filter(|u| u.starts_with("https://"))
        .map(str::to_owned)
        .ok_or_else(|| {
            Error::Download(format!(
                "the newest release of {repo} has no file named like {pattern}"
            ))
        })
}

/// Runs `tool` with `args` in its own process group and streams its output
/// as lines (or, with `capture`, returns stdout instead). Fails on a non-zero
/// exit. Cancelling kills the group; whatever the command left running is
/// killed when it exits.
pub async fn run_command(
    tool: &Tool,
    args: &[String],
    cwd: &Path,
    env: &[(String, String)],
    cancel: &Cancel,
    emit: Emit<'_>,
    capture: bool,
) -> Result<String> {
    let mut cmd = std::process::Command::new(&tool.program);
    cmd.args(&tool.args)
        .args(args)
        .current_dir(cwd)
        .envs(env.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    process_group::configure(&mut cmd);
    let mut child = tokio::process::Command::from(cmd)
        .kill_on_drop(true)
        .spawn()
        .map_err(|source| Error::Io {
            path: tool.program.clone(),
            source,
        })?;
    let Some(pid) = child.id() else {
        return Err(Error::Other(format!(
            "{} exited immediately",
            tool.program.display()
        )));
    };
    // The raw handle is not `Send`; it must be gone before the first await.
    let attached = {
        #[cfg(windows)]
        let handle = child.raw_handle();
        #[cfg(windows)]
        let attached = match handle {
            Some(handle) => ProcessGroup::attach(&SpawnedChild { pid, handle }),
            None => Err(std::io::Error::other("no process handle")),
        };
        #[cfg(not(windows))]
        let attached = ProcessGroup::attach(&SpawnedChild { pid });
        attached
    };
    let group = match attached {
        Ok(group) => group,
        Err(err) => {
            let _ = child.start_kill();
            return Err(Error::Other(format!(
                "could not start {}: {err}",
                tool.program.display()
            )));
        }
    };

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let out_lines = async {
        match stdout {
            Some(s) if capture => read_all(s).await,
            Some(s) => {
                read_lines(s, emit).await;
                String::new()
            }
            None => String::new(),
        }
    };
    let err_lines = async {
        match stderr {
            Some(s) if capture => {
                // A failing query explains itself on stderr: keep it for the error.
                read_all(s).await
            }
            Some(s) => {
                read_lines(s, emit).await;
                String::new()
            }
            None => String::new(),
        }
    };
    let wait = async {
        loop {
            tokio::select! {
                status = child.wait() => return Some(status),
                () = tokio::time::sleep(Duration::from_millis(100)) => {
                    if cancel.load(Ordering::Relaxed) {
                        return None;
                    }
                }
            }
        }
    };
    let wait = async {
        let status = wait.await;
        // Grandchildren keep the pipes open; ending the group ends the readers.
        let _ = group.kill();
        status
    };
    let (status, stdout_text, stderr_text) = tokio::join!(wait, out_lines, err_lines);
    let _ = child.wait().await;
    // The Job Object / process group must be empty before this returns.
    for _ in 0..100 {
        if group.is_empty().unwrap_or(true) {
            break;
        }
        let _ = group.kill();
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    drop(group);

    let Some(status) = status else {
        return Err(Error::Cancelled);
    };
    let status = status.map_err(|source| Error::Io {
        path: tool.program.clone(),
        source,
    })?;
    if status.success() {
        Ok(stdout_text)
    } else {
        let detail = stderr_text.trim();
        Err(Error::Command {
            program: tool
                .program
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            code: status.code(),
            detail: (!detail.is_empty()).then(|| detail.to_owned()),
        })
    }
}

async fn read_all(mut reader: impl AsyncRead + Unpin) -> String {
    let mut buf = Vec::new();
    let _ = reader.read_to_end(&mut buf).await;
    files::strip_ansi(&String::from_utf8_lossy(&buf))
}

/// Emits each complete line; for lines redrawn with `\r` (progress bars)
/// only the last state is kept.
async fn read_lines(mut reader: impl AsyncRead + Unpin, emit: Emit<'_>) {
    let mut pending = Vec::new();
    let mut chunk = [0u8; 8192];
    let flush = |line: &[u8]| {
        let text = String::from_utf8_lossy(line);
        let last = text
            .rsplit('\r')
            .find(|s| !s.trim().is_empty())
            .unwrap_or("");
        let text = files::strip_ansi(last);
        let text = text.trim_end();
        if !text.is_empty() {
            emit(Event::Line {
                text: text.to_owned(),
            });
        }
    };
    loop {
        let n = match reader.read(&mut chunk).await {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        pending.extend_from_slice(&chunk[..n]);
        while let Some(pos) = pending.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = pending.drain(..=pos).collect();
            flush(&line[..line.len() - 1]);
        }
    }
    if !pending.is_empty() {
        flush(&pending);
    }
}
