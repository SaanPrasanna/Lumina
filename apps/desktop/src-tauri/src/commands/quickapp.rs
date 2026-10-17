//! QuickApp: one-click projects from `resources/quickapp/recipes.toml`.
//!
//! One project is created at a time, in the background. Before the recipe
//! runs, whatever it needs and is missing is installed from Quick Add (the
//! newest PHP, Composer, Node.js LTS, the configured database) and the
//! database server is started, so a fresh Lumina can create a Laravel site in
//! one click. Output arrives as `quickapp-output` events and the job's state
//! with `quickapp-changed`. Afterwards the site is picked up like any folder
//! in the projects folder (Phase 4); the web server is started if it is down.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use lumina_config::schema::DatabaseEngine;
use lumina_core::Error;
use lumina_quickapp::recipe::{Need, RecipeOption};
use lumina_quickapp::{Event, Tool, files};
use lumina_services::orchestrator::Orchestrator;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::error::CommandResult;
use crate::events;
use crate::state::{AppState, Runtime};

/// Lines kept for the log panel (the full output is in the log file).
const MAX_LINES: usize = 4000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum JobState {
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    pub index: usize,
    pub total: usize,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
// Only the TypeScript name changes (JSON never carries a struct's name).
#[serde(rename = "QuickAppJob")]
pub struct Job {
    pub id: u64,
    pub recipe: String,
    pub recipe_name: String,
    pub name: String,
    pub dir: PathBuf,
    pub domain: String,
    pub url: String,
    pub state: JobState,
    pub step: Option<Step>,
    pub download: Option<(u64, Option<u64>)>,
    pub error: Option<String>,
    /// The project was created, but something after it did not work (e.g.
    /// the web server could not start because another program holds port 80).
    pub warning: Option<String>,
    pub lines: Vec<String>,
    /// Lines dropped from the front of `lines`.
    pub dropped: usize,
    /// A failed or cancelled run left the project folder behind.
    pub partial: bool,
    /// Created by this job; dropped with "Delete partial project".
    pub created_database: Option<String>,
    pub log_file: PathBuf,
    #[serde(skip)]
    cancel: lumina_packages::Cancel,
}

/// The current (or last) job.
#[derive(Default)]
pub struct Jobs {
    job: Mutex<Option<Job>>,
    next_id: Mutex<u64>,
    /// Lines and progress not yet sent to the UI.
    pending: Mutex<Output>,
}

#[derive(Debug, Default, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
struct Output {
    job_id: u64,
    /// Index of `lines[0]` in the job's whole output.
    first: usize,
    lines: Vec<String>,
    step: Option<Step>,
    download: Option<(u64, Option<u64>)>,
}

impl Jobs {
    fn job(&self) -> std::sync::MutexGuard<'_, Option<Job>> {
        self.job.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn pending(&self) -> std::sync::MutexGuard<'_, Output> {
        self.pending.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn running(&self) -> bool {
        self.job()
            .as_ref()
            .is_some_and(|j| j.state == JobState::Running)
    }
}

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RecipeView {
    pub id: String,
    pub name: String,
    pub description: String,
    pub homepage: String,
    pub category: String,
    pub needs: Vec<Need>,
    /// With each choice's extra needs, so the UI can show what gets installed.
    pub options: Vec<RecipeOption>,
}

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct QuickAppOverview {
    pub recipes: Vec<RecipeView>,
    /// The projects folder.
    pub www: PathBuf,
    pub tld: String,
    /// Needs that are not installed yet (installed automatically on create).
    pub missing: Vec<Need>,
    /// Folder names already used in the projects folder.
    pub taken: Vec<String>,
    pub job: Option<Job>,
    /// How the database is reached, for installers that ask for it.
    pub database: DatabaseInfo,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseInfo {
    pub engine: DatabaseEngine,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
}

fn database_info(cfg: &lumina_config::AppConfig) -> DatabaseInfo {
    DatabaseInfo {
        engine: cfg.database.engine,
        host: "127.0.0.1".into(),
        port: cfg.database.port,
        user: "root".into(),
        password: String::new(),
    }
}

fn db_package(engine: DatabaseEngine) -> &'static str {
    match engine {
        DatabaseEngine::Mysql => "mysql",
        DatabaseEngine::Mariadb => "mariadb",
    }
}

/// Needs from `needs` that are not installed.
async fn missing(rt: &Runtime, needs: &[Need]) -> Vec<Need> {
    let cfg = rt.config();
    let mut out = Vec::new();
    for need in needs {
        let installed = match need {
            Need::Php => rt.stack.php().cli(&cfg.php).await.is_some(),
            Need::Composer => !rt.packages.installed("composer").is_empty(),
            Need::Node => !rt.packages.installed("node").is_empty(),
            Need::Database => !rt
                .packages
                .installed(db_package(cfg.database.engine))
                .is_empty(),
        };
        if !installed {
            out.push(*need);
        }
    }
    out
}

#[tauri::command]
#[specta::specta]
pub async fn quickapp_overview(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<QuickAppOverview> {
    let rt = state.runtime()?;
    let cfg = rt.config();
    let recipes: Vec<RecipeView> = rt
        .recipes
        .recipes
        .iter()
        .map(|r| RecipeView {
            id: r.id.clone(),
            name: r.name.clone(),
            description: r.description.clone(),
            homepage: r.homepage.clone(),
            category: r.category.clone(),
            needs: r.needs.clone(),
            options: r.options.clone(),
        })
        .collect();
    let www = rt.stack.www_root(&cfg);
    let taken = std::fs::read_dir(&www)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.file_name().to_string_lossy().to_lowercase())
                .collect()
        })
        .unwrap_or_default();
    let all = [Need::Php, Need::Composer, Need::Node, Need::Database];
    Ok(QuickAppOverview {
        recipes,
        www,
        tld: cfg.general.tld.clone(),
        missing: missing(&rt, &all).await,
        taken,
        job: app.state::<Jobs>().job().clone(),
        database: database_info(&cfg),
    })
}

/// Starts creating project `name` from `recipe`.
#[tauri::command]
#[specta::specta]
pub fn quickapp_create(
    app: AppHandle,
    state: State<'_, AppState>,
    recipe: String,
    name: String,
    options: BTreeMap<String, String>,
) -> CommandResult<()> {
    let rt = state.runtime()?;
    let cfg = rt.config();
    let jobs = app.state::<Jobs>();
    if jobs.running() {
        return Err(
            Error::Other("another project is being created; wait for it to finish".into()).into(),
        );
    }
    let found = rt
        .recipes
        .recipe(&recipe)
        .ok_or_else(|| Error::Other(format!("unknown recipe `{recipe}`")))?
        .clone();
    let options = found.resolve_options(&options).map_err(Error::from)?;
    let name = name.trim().to_owned();
    if !files::is_valid_name(&name) {
        return Err(Error::Other(
            "use lowercase letters, digits and dashes for the name (it becomes the folder and the domain)".into(),
        )
        .into());
    }
    let www = rt.stack.www_root(&cfg);
    let dir = www.join(&name);
    if dir.exists() {
        return Err(Error::Other(format!("{} already exists", dir.display())).into());
    }
    let domain = lumina_vhost::domain::domain_for(&name, &cfg.general.tld)
        .ok_or_else(|| Error::Other(format!("`{name}` cannot be a domain name")))?;
    let https = cfg.general.auto_ssl && rt.stack.tls_error().is_none();
    let url = match (https, cfg.web.https_port, cfg.web.http_port) {
        (true, 443, _) => format!("https://{domain}"),
        (true, port, _) => format!("https://{domain}:{port}"),
        (false, _, 80) => format!("http://{domain}"),
        (false, _, port) => format!("http://{domain}:{port}"),
    };

    let id = {
        let mut next = jobs.next_id.lock().unwrap_or_else(|e| e.into_inner());
        *next += 1;
        *next
    };
    let log_dir = rt.layout.logs().join("quickapp");
    let _ = std::fs::create_dir_all(&log_dir);
    let cancel = lumina_packages::Cancel::new(AtomicBool::new(false));
    *jobs.job() = Some(Job {
        id,
        recipe: found.id.clone(),
        recipe_name: found.name.clone(),
        name: name.clone(),
        dir: dir.clone(),
        domain: domain.clone(),
        url: url.clone(),
        state: JobState::Running,
        step: None,
        download: None,
        error: None,
        warning: None,
        lines: Vec::new(),
        dropped: 0,
        partial: false,
        created_database: None,
        log_file: log_dir.join(format!("{name}.log")),
        cancel: cancel.clone(),
    });
    *jobs.pending() = Output {
        job_id: id,
        ..Output::default()
    };
    let _ = app.emit(events::QUICKAPP_CHANGED, ());
    log::info!(
        "QuickApp: creating {name} ({}) in {}",
        found.name,
        dir.display()
    );

    let flusher = spawn_flusher(app.clone(), id);
    tauri::async_runtime::spawn(async move {
        let sink = Sink::new(app.clone(), id);
        let outcome = create(
            &app, &rt, &found, &options, &name, &dir, &domain, &url, &cancel, &sink,
        )
        .await;
        let (state, error, created_database) = match outcome {
            Ok(warning) => {
                if let Some(slot) = app.state::<Jobs>().job().as_mut().filter(|j| j.id == id) {
                    slot.warning = warning;
                }
                (JobState::Succeeded, None, None)
            }
            Err((lumina_quickapp::Error::Cancelled, db)) => (JobState::Cancelled, None, db),
            Err((err, db)) => (JobState::Failed, Some(err.to_string()), db),
        };
        match &error {
            None if state == JobState::Succeeded => {
                sink.line(format!("Done: {url}"));
                log::info!("QuickApp: created {name}");
                crate::notify::send(&app, crate::notify::Note::ProjectReady { name: &name });
            }
            None => {
                sink.line("Cancelled.".to_owned());
                log::info!("QuickApp: {name} cancelled");
            }
            Some(err) => {
                sink.line(format!("Failed: {err}"));
                log::warn!("QuickApp: {name} failed: {err}");
                crate::notify::send(&app, crate::notify::Note::ProjectFailed { name: &name });
            }
        }
        flusher.store(true, Ordering::Relaxed);
        flush(&app, id);
        {
            let jobs = app.state::<Jobs>();
            let mut slot = jobs.job();
            if let Some(job) = slot.as_mut().filter(|j| j.id == id) {
                job.state = state;
                job.error = error;
                job.download = None;
                job.partial = state != JobState::Succeeded && job.dir.exists();
                job.created_database = created_database;
            }
        }
        let _ = app.emit(events::QUICKAPP_CHANGED, ());
        let _ = app.emit(events::SITES_CHANGED, ());
    });
    Ok(())
}

/// Everything after validation: installs, services, the recipe, the site.
#[allow(clippy::too_many_arguments)]
async fn create(
    app: &AppHandle,
    rt: &Arc<Runtime>,
    recipe: &lumina_quickapp::Recipe,
    options: &BTreeMap<String, String>,
    name: &str,
    dir: &std::path::Path,
    domain: &str,
    url: &str,
    cancel: &lumina_packages::Cancel,
    sink: &Sink,
) -> Result<Option<String>, (lumina_quickapp::Error, Option<String>)> {
    let fail = |e: lumina_core::Error| (lumina_quickapp::Error::Other(e.to_string()), None);
    let needs = recipe.needs_for(options);

    // 1. What is missing is installed from the catalog.
    for need in missing(rt, &needs).await {
        let id = match need {
            Need::Php => "php",
            Need::Composer => "composer",
            Need::Node => "node",
            Need::Database => db_package(rt.config().database.engine),
        };
        install_newest(app, rt, id, cancel, sink).await?;
    }
    let mut cfg = rt.config();
    rt.stack
        .refresh_shims(&cfg, Some(&rt.shim_exe))
        .await
        .map_err(fail)?;

    // 2. The database server runs.
    let db_kind = Orchestrator::configured_db(&cfg);
    let mut db_version = String::new();
    if needs.contains(&Need::Database) {
        let db = rt.stack.database(cfg.database.engine);
        if db.status().await.runtime.is_none() {
            sink.line(format!("Starting {}...", db.engine().name));
            rt.stack.start(db_kind, &cfg).await.map_err(fail)?;
        }
        if let Some(running) = db.status().await.runtime {
            cfg.database.port = running.port;
            db_version = running.version;
        }
    }

    // 3. The recipe, with Lumina's tools.
    let shims = rt.stack.shims(&cfg).await;
    let tools: BTreeMap<String, Tool> = shims
        .iter()
        .map(|s| {
            (
                s.name.clone(),
                Tool {
                    program: s.program.clone(),
                    args: s.args.clone(),
                },
            )
        })
        .collect();
    let composer_home = rt.layout.etc().join("composer");
    let _ = std::fs::create_dir_all(&composer_home);
    let www = rt.stack.www_root(&cfg);
    let db = files::database_name(name);
    let engine = cfg.database.engine;
    let values: BTreeMap<String, String> = [
        ("name", name.to_owned()),
        ("dir", dir.to_string_lossy().into_owned()),
        ("www", www.to_string_lossy().into_owned()),
        ("domain", domain.to_owned()),
        ("url", url.to_owned()),
        ("db", db),
        ("db_driver", db_package(engine).to_owned()),
        ("db_host", "127.0.0.1".to_owned()),
        ("db_port", cfg.database.port.to_string()),
        ("db_user", "root".to_owned()),
        ("db_password", String::new()),
        (
            "db_server_version",
            match engine {
                DatabaseEngine::Mysql => db_version,
                DatabaseEngine::Mariadb => format!("{db_version}-MariaDB"),
            },
        ),
        ("mail_port", cfg.mailpit.smtp_port.to_string()),
        (
            "composer_home",
            composer_home.to_string_lossy().into_owned(),
        ),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
    .collect();
    let ctx = lumina_quickapp::Context {
        dir: dir.to_path_buf(),
        www: www.clone(),
        values,
        tools,
        env: environment(rt, &shims, &composer_home),
        templates: rt.resources.join("quickapp").join("templates"),
        work_dir: rt.layout.tmp().join("quickapp").join(name),
        proxy: cfg.packages.proxy.clone(),
    };
    let emit = |event: Event| sink.event(event);
    let report = lumina_quickapp::runner::run(recipe, options, &ctx, cancel, &emit).await;
    let _ = std::fs::remove_dir_all(&ctx.work_dir);
    if let Err(err) = report.result {
        return Err((err, report.created_database));
    }

    // 4. The site: its document root if the recipe names one, a running web server.
    if let Some(docroot) = &recipe.docroot {
        let mut cfg = rt.config();
        cfg.sites.entry(name.to_owned()).or_default().docroot = docroot.clone();
        lumina_config::save(&rt.layout.config_file(), &cfg).map_err(fail)?;
        *rt.config.write().unwrap_or_else(|e| e.into_inner()) = cfg;
        let _ = app.emit(events::CONFIG_CHANGED, "quickapp");
    }
    // From here on the project exists and is complete: a problem with the
    // web server is a warning, never a reason to offer deleting the project.
    let cfg = rt.config();
    if rt.stack.running_web().await.is_none() {
        sink.line("Starting the web server...".to_owned());
        if let Err(err) = rt
            .stack
            .start(Orchestrator::configured_web(&cfg), &cfg)
            .await
        {
            let warning = format!("the web server could not start: {err}");
            sink.line(format!("Warning: {warning}"));
            return Ok(Some(warning));
        }
    }
    // The folder watcher also notices the folder; syncing now makes the site
    // ready before "Open site" is clicked.
    if let Err(err) = crate::commands::sites::resync(app, false).await {
        let warning = format!("the site could not be set up yet: {err}");
        sink.line(format!("Warning: {warning}"));
        return Ok(Some(warning));
    }
    Ok(None)
}

/// PATH with Lumina's shims (and Node.js) first; PHP finds its php.ini
/// (PHPRC) even when a tool starts it without `-c`; Composer uses Lumina's
/// own global folder; no colours or prompts.
fn environment(
    rt: &Runtime,
    shims: &[lumina_services::tools::Shim],
    composer_home: &std::path::Path,
) -> Vec<(String, String)> {
    let mut path = vec![rt.stack.shim_dir()];
    if let Some(node) = shims.iter().find(|s| s.name == "node")
        && let Some(dir) = node.program.parent()
    {
        path.push(dir.to_path_buf());
    }
    if let Some(php) = shims.iter().find(|s| s.name == "php")
        && let Some(dir) = php.program.parent()
    {
        path.push(dir.to_path_buf());
    }
    if let Some(system) = std::env::var_os("PATH") {
        path.extend(std::env::split_paths(&system));
    }
    let mut env = vec![(
        "PATH".to_owned(),
        std::env::join_paths(path)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default(),
    )];
    if let Some(php) = shims.iter().find(|s| s.name == "php")
        && let Some(ini) = php.args.get(1)
    {
        env.push(("PHPRC".into(), ini.clone()));
    }
    for (k, v) in [
        (
            "COMPOSER_HOME",
            composer_home.to_string_lossy().into_owned(),
        ),
        ("COMPOSER_NO_INTERACTION", "1".into()),
        ("NO_COLOR", "1".into()),
        ("FORCE_COLOR", "0".into()),
        ("npm_config_update_notifier", "false".into()),
        ("npm_config_fund", "false".into()),
    ] {
        env.push((k.to_owned(), v));
    }
    env
}

/// Installs the newest release of `id` (Node.js and the databases: the newest
/// LTS) and logs its progress.
async fn install_newest(
    app: &AppHandle,
    rt: &Arc<Runtime>,
    id: &str,
    cancel: &lumina_packages::Cancel,
    sink: &Sink,
) -> Result<(), (lumina_quickapp::Error, Option<String>)> {
    let (os, arch) = lumina_packages::catalog::platform();
    let package = rt.packages.catalog().package(id).ok_or_else(|| {
        (
            lumina_quickapp::Error::Other(format!("{id} is not in the catalog")),
            None,
        )
    })?;
    let fitting: Vec<_> = package
        .versions
        .iter()
        .filter(|r| r.fits(os, arch))
        .filter(|r| r.channel.as_deref() != Some("end of life"))
        .collect();
    let release = fitting
        .iter()
        .find(|r| {
            r.channel
                .as_deref()
                .is_some_and(|c| c.to_ascii_lowercase().contains("lts"))
        })
        .or_else(|| fitting.first())
        .ok_or_else(|| {
            (
                lumina_quickapp::Error::Other(format!("no {id} release for this system")),
                None,
            )
        })?;
    sink.event(Event::Step {
        index: 0,
        total: 0,
        label: format!("Install {} {}", package.name, release.version),
    });
    let progress = |p: lumina_packages::Progress| {
        sink.event(Event::Download {
            received: p.received,
            total: p.total,
        });
    };
    let result = rt
        .packages
        .install(id, &release.version, cancel, &progress)
        .await;
    let _ = app.emit(events::PACKAGES_CHANGED, ());
    match result {
        Ok(_) => {
            sink.line(format!("Installed {} {}", package.name, release.version));
            if id == "composer" || id == "php" {
                let _ = rt.stack.refresh_composer_shims(&rt.config()).await;
            }
            Ok(())
        }
        Err(lumina_packages::Error::Cancelled) => Err((lumina_quickapp::Error::Cancelled, None)),
        Err(e) => Err((
            lumina_quickapp::Error::Other(format!(
                "could not install {} {}: {e}",
                package.name, release.version
            )),
            None,
        )),
    }
}

/// Collects output for the job, the log file and the UI.
struct Sink {
    app: AppHandle,
    id: u64,
    log: Mutex<Option<std::fs::File>>,
}

impl Sink {
    fn new(app: AppHandle, id: u64) -> Self {
        let path = app
            .state::<Jobs>()
            .job()
            .as_ref()
            .map(|j| j.log_file.clone());
        let log = path.and_then(|p| std::fs::File::create(p).ok());
        Self {
            app,
            id,
            log: Mutex::new(log),
        }
    }

    fn line(&self, text: String) {
        self.event(Event::Line { text });
    }

    fn event(&self, event: Event) {
        let jobs = self.app.state::<Jobs>();
        let mut slot = jobs.job();
        let Some(job) = slot.as_mut().filter(|j| j.id == self.id) else {
            return;
        };
        let mut pending = jobs.pending();
        match event {
            Event::Line { text } => {
                if let Some(log) = self.log.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
                    let _ = writeln!(log, "{text}");
                }
                job.lines.push(text.clone());
                if job.lines.len() > MAX_LINES {
                    job.lines.remove(0);
                    job.dropped += 1;
                }
                if pending.lines.is_empty() {
                    pending.first = job.dropped + job.lines.len() - 1;
                }
                pending.lines.push(text);
            }
            Event::Step {
                index,
                total,
                label,
            } => {
                if let Some(log) = self.log.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
                    let _ = writeln!(log, "== {label}");
                }
                let step = Step {
                    index,
                    total,
                    label,
                };
                job.step = Some(step.clone());
                job.download = None;
                pending.step = Some(step);
                pending.download = None;
            }
            Event::Download { received, total } => {
                job.download = Some((received, total));
                pending.download = Some((received, total));
            }
        }
    }
}

/// Sends pending output about eight times a second until `stop` is set.
fn spawn_flusher(app: AppHandle, id: u64) -> Arc<AtomicBool> {
    let stop = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stop);
    tauri::async_runtime::spawn(async move {
        while !flag.load(Ordering::Relaxed) {
            tokio::time::sleep(Duration::from_millis(120)).await;
            flush(&app, id);
        }
    });
    stop
}

fn flush(app: &AppHandle, id: u64) {
    let jobs = app.state::<Jobs>();
    let out = {
        let mut pending = jobs.pending();
        if pending.job_id != id
            || (pending.lines.is_empty() && pending.step.is_none() && pending.download.is_none())
        {
            return;
        }
        let out = pending.clone();
        pending.lines.clear();
        pending.step = None;
        pending.download = None;
        out
    };
    let _ = app.emit(events::QUICKAPP_OUTPUT, out);
}

/// Stops the running job: its process tree is killed.
#[tauri::command]
#[specta::specta]
pub fn quickapp_cancel(app: AppHandle) {
    if let Some(job) = app.state::<Jobs>().job().as_ref() {
        job.cancel.store(true, Ordering::Relaxed);
    }
}

/// Forgets a finished job.
#[tauri::command]
#[specta::specta]
pub fn quickapp_dismiss(app: AppHandle) {
    let jobs = app.state::<Jobs>();
    let mut slot = jobs.job();
    if slot.as_ref().is_some_and(|j| j.state != JobState::Running) {
        *slot = None;
    }
    drop(slot);
    let _ = app.emit(events::QUICKAPP_CHANGED, ());
}

/// Removes what a failed or cancelled job left: the project folder and the
/// database it created.
#[tauri::command]
#[specta::specta]
pub async fn quickapp_delete_partial(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let rt = state.runtime()?;
    let (dir, database) = {
        let jobs = app.state::<Jobs>();
        let slot = jobs.job();
        let job = slot
            .as_ref()
            .filter(|j| matches!(j.state, JobState::Failed | JobState::Cancelled))
            .ok_or_else(|| Error::Other("nothing to delete".into()))?;
        (job.dir.clone(), job.created_database.clone())
    };
    if dir.exists() {
        let target = dir.clone();
        tauri::async_runtime::spawn_blocking(move || {
            std::fs::remove_dir_all(lumina_platform::long_path::long(&target))
        })
        .await
        .map_err(|e| Error::Other(e.to_string()))?
        .map_err(|source| Error::Io {
            path: dir.clone(),
            source,
        })?;
        log::info!("QuickApp: removed {}", dir.display());
    }
    if let Some(db) = database.filter(|d| files::is_safe_database(d)) {
        let cfg = rt.config();
        let shims = rt.stack.shims(&cfg).await;
        if let Some(mysql) = shims.iter().find(|s| s.name == "mysql") {
            let tool = Tool {
                program: mysql.program.clone(),
                args: mysql.args.clone(),
            };
            let args = ["-e".to_owned(), files::sql_drop(&db)];
            let cancel = lumina_packages::Cancel::new(AtomicBool::new(false));
            match lumina_quickapp::runner::run_command(
                &tool,
                &args,
                &rt.stack.www_root(&cfg),
                &[],
                &cancel,
                &|_| {},
                true,
            )
            .await
            {
                Ok(_) => log::info!("QuickApp: dropped database {db}"),
                Err(err) => {
                    return Err(Error::Other(format!(
                        "the folder was removed, but the database `{db}` could not be dropped: {err}"
                    ))
                    .into());
                }
            }
        }
    }
    {
        let jobs = app.state::<Jobs>();
        if let Some(job) = jobs.job().as_mut() {
            job.partial = false;
            job.created_database = None;
        }
    }
    let _ = app.emit(events::QUICKAPP_CHANGED, ());
    let _ = app.emit(events::SITES_CHANGED, ());
    Ok(())
}
