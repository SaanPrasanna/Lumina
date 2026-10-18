//! Dev servers of app sites (Vite, Next.js, Nuxt, Django, FastAPI, ...).
//!
//! Each runs under the supervisor as `app:<folder>`, in its project folder,
//! with Lumina's PATH shims first on PATH (so `node`, `npm` and `python` are
//! the versions chosen in Lumina) and `PORT`/`HOST` set. The web server
//! forwards `<name>.test` to its port.
//!
//! The command is written to a small script (`etc/apps/<name>.cmd` or `.sh`)
//! and that script is run: `cmd /C` cannot take a command with quotes as one
//! argument, and a script needs no quoting at all.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use lumina_core::paths::Layout;
use lumina_core::types::ServiceStatus;
use lumina_core::{Error, Result};
use lumina_process::{ProcessSpec, ProcessState, RestartPolicy, Supervisor, logs};
use lumina_vhost::Site;
use serde::Serialize;
use tokio::sync::Mutex;
use tokio::time::{Instant, sleep, timeout};

use crate::web;

/// Supervisor IDs are `app:<folder name>`.
pub const PROCESS_PREFIX: &str = "app:";
/// First starts compile a lot (Next.js, Nuxt); listening can take a while.
const READY_TIMEOUT: Duration = Duration::from_secs(120);
const STOP_TIMEOUT: Duration = Duration::from_secs(10);
/// Playwright downloads several hundred megabytes of browsers.
const PLAYWRIGHT_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// A site's dev server, for the Sites page.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    /// The project folder's name (the site's key).
    pub name: String,
    pub domain: String,
    pub framework: String,
    pub command: String,
    pub port: u16,
    pub status: ServiceStatus,
    pub pid: Option<u32>,
    /// Why the last start failed or the server stopped.
    pub error: Option<String>,
    pub log_file: PathBuf,
    /// Runtimes picked by the project's version files at the last start,
    /// e.g. "Node.js 22.11.0 (.nvmrc)".
    pub runtimes: Vec<String>,
}

#[derive(Clone)]
pub struct Apps {
    inner: Arc<Inner>,
}

struct Inner {
    supervisor: Supervisor,
    layout: Layout,
    /// Last start error per site.
    errors: Mutex<BTreeMap<String, String>>,
    /// Sites whose dev server was started and not stopped since.
    started: Mutex<BTreeSet<String>>,
    /// Runtimes each site's version files picked at its last start.
    runtimes: Mutex<BTreeMap<String, Vec<String>>>,
}

pub fn process_id(name: &str) -> String {
    format!("{PROCESS_PREFIX}{name}")
}

impl Apps {
    pub fn new(supervisor: Supervisor, layout: Layout) -> Self {
        Self {
            inner: Arc::new(Inner {
                supervisor,
                layout,
                errors: Mutex::new(BTreeMap::new()),
                started: Mutex::new(BTreeSet::new()),
                runtimes: Mutex::new(BTreeMap::new()),
            }),
        }
    }

    pub fn log_file(&self, name: &str) -> PathBuf {
        self.inner
            .layout
            .logs()
            .join("apps")
            .join(format!("{name}.log"))
    }

    /// Starts `site`'s dev server and waits until it accepts connections.
    /// `path_first` goes in front of PATH (Lumina's shim folder).
    pub async fn start(&self, site: &Site, path_first: &Path) -> Result<()> {
        let result = self.start_inner(site, path_first).await;
        let mut errors = self.inner.errors.lock().await;
        match &result {
            Ok(()) => errors.remove(&site.name),
            Err(err) => errors.insert(site.name.clone(), err.to_string()),
        };
        result
    }

    async fn start_inner(&self, site: &Site, path_first: &Path) -> Result<()> {
        let id = process_id(&site.name);
        let Some(app) = &site.app else {
            return Err(Error::Service(
                id,
                format!("{} is served by PHP, not by a dev server", site.domain),
            ));
        };
        if let Some(info) = self.inner.supervisor.info(&id).await
            && info.state != ProcessState::Failed
        {
            return Err(Error::Service(id, "already running".into()));
        }
        if app.command.trim().is_empty() {
            return Err(Error::Service(
                id,
                format!(
                    "no dev server command is set for {}; set one in the site's settings (use {{port}} for its port)",
                    site.domain
                ),
            ));
        }
        if site.path.join("package.json").is_file() && !site.path.join("node_modules").is_dir() {
            return Err(Error::Service(
                id,
                format!(
                    "the dependencies of {} are not installed yet; run `npm install` (or pnpm, yarn, bun) in {} first",
                    site.name,
                    site.path.display()
                ),
            ));
        }
        web::check_port(app.port)?;
        let runtimes = self.project_runtimes(&site.path)?;
        let mut path: Vec<PathBuf> = runtimes.iter().map(|(dir, _)| dir.clone()).collect();
        path.push(path_first.to_path_buf());
        self.inner.runtimes.lock().await.insert(
            site.name.clone(),
            runtimes.into_iter().map(|(_, text)| text).collect(),
        );

        let script = self.write_script(&site.name, &app.command)?;
        let (program, args) = shell(&script);
        let spec = ProcessSpec {
            id: id.clone(),
            program,
            args,
            env: env(&path, app.port, &self.ca_cert()),
            cwd: Some(site.path.clone()),
            log_file: self.log_file(&site.name),
            // A dev server that exits has usually failed to compile or start;
            // starting it again would fail the same way.
            restart: RestartPolicy::Never,
        };
        self.inner.supervisor.start(spec).await?;
        self.inner.started.lock().await.insert(site.name.clone());

        if !self.wait_ready(&id, app.port).await {
            let _ = self.inner.supervisor.stop(&id, STOP_TIMEOUT).await;
            return Err(Error::Service(
                id,
                format!(
                    "the dev server of {} did not start listening on port {}.\n{}",
                    site.name,
                    app.port,
                    logs::tail(&self.log_file(&site.name), 15)
                ),
            ));
        }
        tracing::info!(
            "{} dev server for {} on 127.0.0.1:{}",
            app.framework,
            site.domain,
            app.port
        );
        Ok(())
    }

    /// The Node.js and Python folders the project's version files ask for
    /// (`.nvmrc`, `.python-version`, ...), to go on PATH before the shims,
    /// each with a description. Fails when a requested version is not
    /// installed, rather than silently running another one.
    fn project_runtimes(&self, dir: &Path) -> Result<Vec<(PathBuf, String)>> {
        let requested = lumina_vhost::app::requested_versions(dir);
        let mut out = Vec::new();
        for (tool, label, wanted) in [
            ("node", "Node.js", requested.node),
            ("python", "Python", requested.python),
        ] {
            let Some((version, file)) = wanted else {
                continue;
            };
            let installs = lumina_packages::installed::list(&self.inner.layout.bin().join(tool));
            let Some(install) = installs
                .iter()
                .find(|i| i.version == version || i.version.starts_with(&format!("{version}.")))
            else {
                return Err(Error::Service(
                    process_id(&dir.file_name().unwrap_or_default().to_string_lossy()),
                    format!(
                        "the project asks for {label} {version} ({file}), which is not installed; install it from Quick Add"
                    ),
                ));
            };
            // Windows builds keep the programs at the top; Unix ones in bin/.
            let bin = if install.dir.join("bin").is_dir() {
                install.dir.join("bin")
            } else {
                install.dir.clone()
            };
            out.push((bin, format!("{label} {} ({file})", install.version)));
        }
        Ok(out)
    }

    /// Lumina's CA certificate, so Node.js trusts `https://*.test` (fetch,
    /// SSR requests, Playwright), if HTTPS has been set up.
    fn ca_cert(&self) -> Option<PathBuf> {
        let ca = self.inner.layout.ssl().join("ca.crt");
        ca.is_file().then_some(ca)
    }

    /// `npx playwright install` in the project, with Lumina's Node.js: the
    /// browsers Playwright tests need. Output goes to
    /// `logs/apps/<name>-playwright.log`; the error carries its end.
    pub async fn playwright_install(&self, site: &Site, path_first: &Path) -> Result<()> {
        let id = process_id(&site.name);
        if !site.path.join("package.json").is_file() {
            return Err(Error::Service(
                id,
                format!(
                    "{} has no package.json, so it has no Playwright tests",
                    site.name
                ),
            ));
        }
        let name = format!("{}-playwright", site.name);
        let script = self.write_script(&name, "npx --yes playwright install")?;
        let (program, args) = shell(&script);
        let log = self.log_file(&name);
        let out =
            logs::open_for_run(&log, "Lumina: npx --yes playwright install").map_err(|source| {
                Error::Io {
                    path: log.clone(),
                    source,
                }
            })?;
        let err = out.try_clone().map_err(|source| Error::Io {
            path: log.clone(),
            source,
        })?;
        let mut cmd = tokio::process::Command::new(program);
        cmd.args(args)
            .current_dir(&site.path)
            .envs(env(&[path_first.to_path_buf()], 0, &self.ca_cert()))
            .stdin(std::process::Stdio::null())
            .stdout(out)
            .stderr(err)
            .kill_on_drop(true);
        let mut child = cmd.spawn().map_err(|source| Error::Io {
            path: script.clone(),
            source,
        })?;
        let status = timeout(PLAYWRIGHT_TIMEOUT, child.wait())
            .await
            .map_err(|_| {
                Error::Service(
                    id.clone(),
                    "Playwright's download took over 30 minutes".into(),
                )
            })?
            .map_err(|source| Error::Io {
                path: script.clone(),
                source,
            })?;
        if status.success() {
            Ok(())
        } else {
            Err(Error::Service(
                id,
                format!(
                    "installing Playwright's browsers failed.\n{}",
                    logs::tail(&log, 15)
                ),
            ))
        }
    }

    /// Until the port answers; gives up early when the process has exited.
    async fn wait_ready(&self, id: &str, port: u16) -> bool {
        let deadline = Instant::now() + READY_TIMEOUT;
        while Instant::now() < deadline {
            let connect = tokio::net::TcpStream::connect(("127.0.0.1", port));
            if let Ok(Ok(_)) = timeout(Duration::from_millis(250), connect).await {
                return true;
            }
            match self.inner.supervisor.info(id).await {
                Some(info) if info.state != ProcessState::Failed => {}
                _ => return false,
            }
            sleep(Duration::from_millis(250)).await;
        }
        false
    }

    fn write_script(&self, name: &str, command: &str) -> Result<PathBuf> {
        let dir = self.inner.layout.etc().join("apps");
        let (file, text) = if cfg!(windows) {
            (
                dir.join(format!("{name}.cmd")),
                format!(
                    "@echo off\r\nrem Written by Lumina for {name}; edit the command in its site settings.\r\n{command}\r\n"
                ),
            )
        } else {
            (
                dir.join(format!("{name}.sh")),
                format!(
                    "#!/bin/sh\n# Written by Lumina for {name}; edit the command in its site settings.\nexec {command}\n"
                ),
            )
        };
        crate::template::write_if_changed(&file, text.as_bytes())?;
        Ok(file)
    }

    pub async fn stop(&self, name: &str) -> Result<()> {
        self.inner.errors.lock().await.remove(name);
        self.inner
            .supervisor
            .stop(&process_id(name), STOP_TIMEOUT)
            .await?;
        self.inner.started.lock().await.remove(name);
        Ok(())
    }

    /// Stops every dev server that was started. Returns the failures.
    pub async fn stop_all(&self) -> Vec<(String, Error)> {
        let names: Vec<String> = self.inner.started.lock().await.iter().cloned().collect();
        let mut failures = Vec::new();
        for name in names {
            if let Err(err) = self.stop(&name).await {
                failures.push((name, err));
            }
        }
        failures
    }

    pub async fn is_running(&self, name: &str) -> bool {
        self.inner
            .supervisor
            .info(&process_id(name))
            .await
            .is_some_and(|i| i.state == ProcessState::Running)
    }

    /// The dev servers of every app site in `sites`.
    pub async fn status(&self, sites: &[Site]) -> Vec<AppStatus> {
        let errors = self.inner.errors.lock().await.clone();
        let runtimes = self.inner.runtimes.lock().await.clone();
        let mut out = Vec::new();
        for site in sites {
            let Some(app) = &site.app else { continue };
            let info = self.inner.supervisor.info(&process_id(&site.name)).await;
            let error = errors.get(&site.name).cloned();
            out.push(AppStatus {
                name: site.name.clone(),
                domain: site.domain.clone(),
                framework: app.framework.clone(),
                command: app.command.clone(),
                port: app.port,
                status: crate::service_status(info.as_ref(), error.is_some()),
                pid: info.as_ref().map(|i| i.pid),
                error,
                log_file: self.log_file(&site.name),
                runtimes: runtimes.get(&site.name).cloned().unwrap_or_default(),
            });
        }
        out
    }
}

/// How to run a script: `cmd /D /C <file>` on Windows, `sh <file>` elsewhere.
fn shell(script: &Path) -> (PathBuf, Vec<OsString>) {
    if cfg!(windows) {
        let cmd =
            std::env::var_os("ComSpec").map_or_else(|| PathBuf::from("cmd.exe"), PathBuf::from);
        (
            cmd,
            vec!["/D".into(), "/C".into(), script.as_os_str().to_owned()],
        )
    } else {
        (
            PathBuf::from("/bin/sh"),
            vec![script.as_os_str().to_owned()],
        )
    }
}

/// Lumina's shims first on PATH, the port in PORT (frameworks that read it),
/// no browser opened by the dev server, and Python output not buffered so the
/// log shows it at once.
fn env(path_first: &[PathBuf], port: u16, ca: &Option<PathBuf>) -> Vec<(OsString, OsString)> {
    let existing = std::env::var_os("PATH");
    let dirs = path_first
        .iter()
        .cloned()
        .chain(existing.iter().flat_map(std::env::split_paths));
    let path = std::env::join_paths(dirs).unwrap_or_default();
    let mut env: Vec<(OsString, OsString)> = vec![
        ("PATH".into(), path),
        ("PORT".into(), port.to_string().into()),
        ("HOST".into(), "127.0.0.1".into()),
        ("BROWSER".into(), "none".into()),
        ("PYTHONUNBUFFERED".into(), "1".into()),
    ];
    if let Some(ca) = ca {
        // Added to Node's own list, so public HTTPS keeps working too.
        env.push(("NODE_EXTRA_CA_CERTS".into(), ca.as_os_str().to_owned()));
    }
    env
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_files_pick_installed_runtimes_or_fail() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let layout = Layout::new(tmp.path().join("Lumina"));
        std::fs::create_dir_all(layout.bin().join("node/22.11.0/bin")).expect("node");
        std::fs::create_dir_all(layout.bin().join("python/3.13.0")).expect("python");
        let apps = Apps::new(
            Supervisor::new(lumina_process::ledger::Ledger::disabled()),
            layout.clone(),
        );
        let project = tmp.path().join("front");
        std::fs::create_dir_all(&project).expect("project");
        std::fs::write(project.join(".nvmrc"), "22\n").expect("nvmrc");
        std::fs::write(project.join(".python-version"), "3.13\n").expect("python-version");

        let found = apps.project_runtimes(&project).expect("installed");
        assert_eq!(
            found,
            [
                (
                    layout.bin().join("node/22.11.0/bin"),
                    "Node.js 22.11.0 (.nvmrc)".to_owned()
                ),
                (
                    layout.bin().join("python/3.13.0"),
                    "Python 3.13.0 (.python-version)".to_owned()
                ),
            ]
        );

        std::fs::write(project.join(".nvmrc"), "20\n").expect("nvmrc");
        let err = apps.project_runtimes(&project).expect_err("20 is missing");
        assert!(err.to_string().contains("Node.js 20 (.nvmrc)"), "{err}");
    }

    #[test]
    fn shims_come_first_on_path() {
        let env = env(
            &[PathBuf::from("/lumina/bin/.shims")],
            41234,
            &Some(PathBuf::from("/lumina/ssl/ca.crt")),
        );
        let get = |k: &str| env.iter().find(|(key, _)| key == k).map(|(_, v)| v.clone());
        let path = get("PATH").expect("PATH").to_string_lossy().into_owned();
        assert!(path.starts_with("/lumina/bin/.shims"));
        assert_eq!(get("PORT"), Some("41234".into()));
        assert_eq!(
            get("NODE_EXTRA_CA_CERTS"),
            Some("/lumina/ssl/ca.crt".into())
        );
    }
}
