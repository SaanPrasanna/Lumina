//! Quick Add: list catalog, install/remove versions, switch active version.
//!
//! Installs run in the background; progress arrives as `package-progress`
//! events and the list is refreshed with `packages-changed` when a job ends.
//! A version cannot be removed while it runs or while the settings pin it.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use lumina_config::AppConfig;
use lumina_config::schema::DatabaseEngine;
use lumina_core::Error;
use lumina_core::types::ServiceKind;
use lumina_packages::{Kind, Progress, Stage};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::commands::sites;
use crate::error::CommandResult;
use crate::events;
use crate::state::{AppState, Runtime};

/// A running or finished install.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
// Only the TypeScript name changes (JSON never carries a struct's name).
#[serde(rename = "PackageJob")]
pub struct Job {
    pub id: String,
    pub version: String,
    /// `None` while waiting to start.
    pub stage: Option<Stage>,
    pub received: u64,
    pub total: Option<u64>,
    pub error: Option<String>,
    pub done: bool,
    #[serde(skip)]
    cancel: lumina_packages::Cancel,
}

/// Install jobs by `id@version`.
#[derive(Default)]
pub struct Jobs(Mutex<HashMap<String, Job>>);

impl Jobs {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Job>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn key(id: &str, version: &str) -> String {
    format!("{id}@{version}")
}

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InstalledVersion {
    pub version: String,
    /// Why it cannot be removed right now, if it cannot.
    pub in_use: Option<String>,
    /// Installed by Quick Add (otherwise unpacked by hand).
    pub managed: bool,
    pub requires_php: Option<String>,
}

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AvailableVersion {
    pub version: String,
    pub channel: Option<String>,
    pub size: Option<u64>,
    pub requires_php: Option<String>,
    pub installed: bool,
    /// Bytes of an interrupted download that an install resumes.
    pub partial: Option<u64>,
    /// For phpMyAdmin: whether it runs on the active PHP.
    pub compatible: bool,
}

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PackageView {
    pub id: String,
    pub name: String,
    pub kind: Kind,
    pub description: String,
    pub homepage: String,
    /// The configured choice: "" = newest (phpMyAdmin: newest compatible).
    pub selected: String,
    /// The version that is used: the running one, else the selected one.
    pub active: Option<String>,
    pub running: Option<String>,
    pub installed: Vec<InstalledVersion>,
    pub available: Vec<AvailableVersion>,
    pub jobs: Vec<Job>,
}

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PackagesOverview {
    pub packages: Vec<PackageView>,
    /// The PHP version phpMyAdmin is matched against.
    pub php_version: Option<String>,
    /// The phpMyAdmin served at /phpmyadmin/, if any.
    pub db_ui: Option<String>,
    pub catalog_updated: String,
}

/// The config field that selects a version of `id`.
pub(crate) fn selected(cfg: &AppConfig, id: &str) -> Option<String> {
    Some(match id {
        "php" => cfg.php.version.clone(),
        "nginx" => cfg.web.nginx_version.clone(),
        "apache" => cfg.web.apache_version.clone(),
        "mysql" => cfg.database.mysql_version.clone(),
        "mariadb" => cfg.database.mariadb_version.clone(),
        "redis" => cfg.redis.version.clone(),
        "postgresql" => cfg.postgresql.version.clone(),
        "node" => cfg.tools.node.clone(),
        "python" => cfg.tools.python.clone(),
        "composer" => cfg.tools.composer.clone(),
        "phpmyadmin" => cfg.tools.phpmyadmin.clone(),
        "ngrok" => cfg.tools.ngrok.clone(),
        _ => return None,
    })
}

fn set_selected(cfg: &mut AppConfig, id: &str, version: String) -> lumina_core::Result<()> {
    let slot = match id {
        "php" => &mut cfg.php.version,
        "nginx" => &mut cfg.web.nginx_version,
        "apache" => &mut cfg.web.apache_version,
        "mysql" => &mut cfg.database.mysql_version,
        "mariadb" => &mut cfg.database.mariadb_version,
        "redis" => &mut cfg.redis.version,
        "postgresql" => &mut cfg.postgresql.version,
        "node" => &mut cfg.tools.node,
        "python" => &mut cfg.tools.python,
        "composer" => &mut cfg.tools.composer,
        "phpmyadmin" => &mut cfg.tools.phpmyadmin,
        "ngrok" => &mut cfg.tools.ngrok,
        other => return Err(Error::Other(format!("unknown package `{other}`"))),
    };
    *slot = version;
    Ok(())
}

/// The service a package is, if it is one.
fn service(id: &str) -> Option<ServiceKind> {
    Some(match id {
        "php" => ServiceKind::PhpFcgi,
        "nginx" => ServiceKind::Nginx,
        "apache" => ServiceKind::Apache,
        "mysql" => ServiceKind::Mysql,
        "mariadb" => ServiceKind::Mariadb,
        "redis" => ServiceKind::Redis,
        "postgresql" => ServiceKind::Postgresql,
        _ => return None,
    })
}

async fn running_version(rt: &Runtime, id: &str) -> Option<String> {
    let stack = &rt.stack;
    match id {
        "php" => stack.php().status().await.runtime.map(|r| r.version),
        "nginx" => stack
            .web_status(ServiceKind::Nginx)
            .await
            .runtime
            .map(|r| r.version),
        "apache" => stack
            .web_status(ServiceKind::Apache)
            .await
            .runtime
            .map(|r| r.version),
        "mysql" => stack
            .database(DatabaseEngine::Mysql)
            .status()
            .await
            .runtime
            .map(|r| r.version),
        "mariadb" => stack
            .database(DatabaseEngine::Mariadb)
            .status()
            .await
            .runtime
            .map(|r| r.version),
        "redis" => stack.redis().status().await.runtime.map(|r| r.version),
        "postgresql" => stack.postgresql().status().await.runtime.map(|r| r.version),
        _ => None,
    }
}

/// `wanted` among `versions` (newest first): "" = newest, prefix or exact.
pub(crate) fn pick<'a>(versions: &'a [String], wanted: &str) -> Option<&'a String> {
    let wanted = wanted.trim();
    if wanted.is_empty() {
        return versions.first();
    }
    versions
        .iter()
        .find(|v| *v == wanted || v.starts_with(&format!("{wanted}.")))
}

async fn view(
    rt: &Runtime,
    jobs: &Jobs,
    package: &lumina_packages::Package,
    cfg: &AppConfig,
    php: Option<&str>,
    db_ui: Option<&str>,
) -> PackageView {
    let pm = &rt.packages;
    let installed = pm.installed(&package.id);
    let versions: Vec<String> = installed.iter().map(|i| i.version.clone()).collect();
    let selected = selected(cfg, &package.id).unwrap_or_default();
    let running = running_version(rt, &package.id).await;
    let active = if package.id == "phpmyadmin" {
        db_ui.map(str::to_owned)
    } else {
        running
            .clone()
            .or_else(|| pick(&versions, &selected).cloned())
    };

    let installed_views = installed
        .iter()
        .map(|i| {
            let in_use = if running.as_deref() == Some(i.version.as_str()) {
                Some("running".to_owned())
            } else if !selected.is_empty() && selected == i.version {
                Some("selected".to_owned())
            } else {
                None
            };
            InstalledVersion {
                version: i.version.clone(),
                in_use,
                managed: i.meta.is_some(),
                requires_php: i.meta.as_ref().and_then(|m| m.requires.php.clone()),
            }
        })
        .collect();

    let (os, arch) = lumina_packages::catalog::platform();
    let available = package
        .versions
        .iter()
        .filter(|r| r.fits(os, arch))
        .map(|r| AvailableVersion {
            version: r.version.clone(),
            channel: r.channel.clone(),
            size: r.size,
            requires_php: r.requires.php.clone(),
            installed: versions.contains(&r.version),
            partial: pm.partial_download(&package.id, &r.version),
            compatible: php.is_none_or(|php| {
                lumina_packages::compat::php_satisfies(php, r.requires.php.as_deref())
            }),
        })
        .collect();

    let jobs = jobs
        .lock()
        .values()
        .filter(|j| j.id == package.id)
        .cloned()
        .collect();

    PackageView {
        id: package.id.clone(),
        name: package.name.clone(),
        kind: package.kind,
        description: package.description.clone(),
        homepage: package.homepage.clone(),
        selected,
        active,
        running,
        installed: installed_views,
        available,
        jobs,
    }
}

#[tauri::command]
#[specta::specta]
pub async fn packages_list(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<PackagesOverview> {
    let rt = state.runtime()?;
    let cfg = rt.config();
    let jobs = app.state::<Jobs>();
    let php = rt.stack.php().active_version(&cfg.php).await;
    let db_ui = rt.stack.db_ui(&cfg).await.map(|u| u.version);
    let mut packages = Vec::new();
    for package in &rt.packages.catalog().packages {
        packages.push(view(&rt, &jobs, package, &cfg, php.as_deref(), db_ui.as_deref()).await);
    }
    Ok(PackagesOverview {
        packages,
        php_version: php,
        db_ui,
        catalog_updated: rt.packages.catalog().updated.clone(),
    })
}

#[derive(Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
struct ProgressEvent {
    id: String,
    version: String,
    stage: Stage,
    received: u64,
    total: Option<u64>,
}

/// Starts installing `id` `version` in the background.
#[tauri::command]
#[specta::specta]
pub fn package_install(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    version: String,
) -> CommandResult<()> {
    let rt = state.runtime()?;
    if rt.packages.catalog().release(&id, &version).is_none() {
        return Err(
            Error::Other(format!("{id} {version} is not available for this system")).into(),
        );
    }
    let jobs = app.state::<Jobs>();
    let job_key = key(&id, &version);
    let cancel = lumina_packages::Cancel::new(AtomicBool::new(false));
    {
        let mut map = jobs.lock();
        if map.get(&job_key).is_some_and(|j| !j.done) {
            return Err(Error::Other(format!("{id} {version} is already being installed")).into());
        }
        map.insert(
            job_key.clone(),
            Job {
                id: id.clone(),
                version: version.clone(),
                stage: None,
                received: 0,
                total: None,
                error: None,
                done: false,
                cancel: cancel.clone(),
            },
        );
    }
    let _ = app.emit(events::PACKAGES_CHANGED, ());

    tauri::async_runtime::spawn(async move {
        let last_emit = Mutex::new(Instant::now() - Duration::from_secs(1));
        let last_stage = Mutex::new(None);
        let progress = |p: Progress| {
            let jobs = app.state::<Jobs>();
            if let Some(job) = jobs.lock().get_mut(&job_key) {
                job.stage = Some(p.stage);
                job.received = p.received;
                job.total = p.total;
            }
            // At most ~8 events a second, plus every stage change.
            let mut last = last_emit.lock().unwrap_or_else(|e| e.into_inner());
            let mut stage = last_stage.lock().unwrap_or_else(|e| e.into_inner());
            if *stage != Some(p.stage) || last.elapsed() > Duration::from_millis(120) {
                *last = Instant::now();
                *stage = Some(p.stage);
                let _ = app.emit(
                    events::PACKAGE_PROGRESS,
                    ProgressEvent {
                        id: id.clone(),
                        version: version.clone(),
                        stage: p.stage,
                        received: p.received,
                        total: p.total,
                    },
                );
            }
        };
        let result = rt.packages.install(&id, &version, &cancel, &progress).await;
        let error = match &result {
            Ok(_) => None,
            Err(lumina_packages::Error::Cancelled) => Some("cancelled".to_owned()),
            Err(e) => Some(e.to_string()),
        };
        let package = rt
            .packages
            .catalog()
            .package(&id)
            .map_or_else(|| id.clone(), |p| p.name.clone());
        match &error {
            None => {
                log::info!("installed {id} {version}");
                crate::notify::send(
                    &app,
                    crate::notify::Note::Installed {
                        package: &package,
                        version: &version,
                    },
                );
            }
            Some(e) => {
                log::warn!("{id} {version} not installed: {e}");
                if !matches!(result, Err(lumina_packages::Error::Cancelled)) {
                    crate::notify::send(
                        &app,
                        crate::notify::Note::InstallFailed {
                            package: &package,
                            version: &version,
                        },
                    );
                }
            }
        }
        {
            let jobs = app.state::<Jobs>();
            let mut map = jobs.lock();
            if error.is_none() {
                map.remove(&job_key);
            } else if let Some(job) = map.get_mut(&job_key) {
                job.done = true;
                job.error = error;
            }
        }
        if result.is_ok() {
            after_change(&app, &rt, &id).await;
        }
        let _ = app.emit(events::PACKAGES_CHANGED, ());
    });
    Ok(())
}

/// Stops an install; the partial download is kept so the next install resumes.
#[tauri::command]
#[specta::specta]
pub fn package_cancel(app: AppHandle, id: String, version: String) {
    if let Some(job) = app.state::<Jobs>().lock().get(&key(&id, &version)) {
        job.cancel.store(true, Ordering::Relaxed);
    }
}

/// Forgets a failed or cancelled job (the "x" on its message).
#[tauri::command]
#[specta::specta]
pub fn package_dismiss(app: AppHandle, id: String, version: String) {
    let jobs = app.state::<Jobs>();
    let mut map = jobs.lock();
    if map.get(&key(&id, &version)).is_some_and(|j| j.done) {
        map.remove(&key(&id, &version));
    }
    drop(map);
    let _ = app.emit(events::PACKAGES_CHANGED, ());
}

#[tauri::command]
#[specta::specta]
pub async fn package_uninstall(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    version: String,
) -> CommandResult<()> {
    let rt = state.runtime()?;
    let cfg = rt.config();
    if running_version(&rt, &id).await.as_deref() == Some(version.as_str()) {
        return Err(Error::Other(format!("{id} {version} is running; stop it first")).into());
    }
    let profiles = crate::commands::profiles::referencing(&cfg, &id, &version);
    if !profiles.is_empty() {
        return Err(Error::Other(format!(
            "{id} {version} is used by the profile(s) {}; change them first",
            profiles.join(", ")
        ))
        .into());
    }
    if selected(&cfg, &id).is_some_and(|s| s == version) {
        return Err(Error::Other(format!(
            "{id} {version} is selected in the settings; choose another version first"
        ))
        .into());
    }
    rt.packages
        .uninstall(&id, &version)
        .map_err(lumina_core::Error::from)?;
    after_change(&app, &rt, &id).await;
    let _ = app.emit(events::PACKAGES_CHANGED, ());
    Ok(())
}

/// Selects `version` ("" = newest) and applies it: a running service is
/// restarted on the new version.
#[tauri::command]
#[specta::specta]
pub async fn package_set_active(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    version: String,
) -> CommandResult<()> {
    set_active(&app, &state.runtime()?, &id, version).await
}

/// [`package_set_active`], shared with the tray.
pub async fn set_active(
    app: &AppHandle,
    rt: &Arc<Runtime>,
    id: &str,
    version: String,
) -> CommandResult<()> {
    let mut cfg = rt.config();
    set_selected(&mut cfg, id, version)?;
    let warnings = lumina_config::validate::validate(&mut cfg);
    lumina_config::save(&rt.layout.config_file(), &cfg)?;
    *rt.config.write().unwrap_or_else(|e| e.into_inner()) = cfg.clone();
    *rt.warnings.write().unwrap_or_else(|e| e.into_inner()) = warnings;
    let _ = app.emit(events::CONFIG_CHANGED, "packages");

    if let Some(kind) = service(id) {
        let running = match kind {
            ServiceKind::Nginx | ServiceKind::Apache => {
                rt.stack.web_status(kind).await.runtime.is_some()
            }
            _ => running_version(rt, id).await.is_some(),
        };
        if running {
            rt.stack.restart(kind, &cfg).await?;
        }
    }
    after_change(app, rt, id).await;
    let _ = app.emit(events::PACKAGES_CHANGED, ());
    Ok(())
}

/// Follow-up work after a version of `id` appeared, disappeared or was chosen.
async fn after_change(app: &AppHandle, rt: &Arc<Runtime>, id: &str) {
    // Any version change can change what `php`, `node`, `mysql`... run.
    let _ = rt;
    crate::commands::env_path::refresh_shims_later(app);
    if id == "phpmyadmin" || id == "php" {
        // /phpmyadmin/ follows the installed versions and the active PHP.
        sites::resync_later(app);
    }
}

/// Opens phpMyAdmin at http(s)://localhost/phpmyadmin/.
#[tauri::command]
#[specta::specta]
pub async fn db_ui_open(app: AppHandle, state: State<'_, AppState>) -> CommandResult<()> {
    open_db_ui(&app, &*state.runtime()?).await
}

/// [`db_ui_open`], shared with the tray.
pub async fn open_db_ui(app: &AppHandle, rt: &Runtime) -> CommandResult<()> {
    let cfg = rt.config();
    if rt.packages.installed("phpmyadmin").is_empty() {
        return Err(Error::Other("phpMyAdmin is not installed; add it in Quick Add".into()).into());
    }
    if rt.stack.db_ui(&cfg).await.is_none() {
        return Err(Error::Other(
            "no installed phpMyAdmin runs on the active PHP version; install a compatible one in Quick Add".into(),
        )
        .into());
    }
    if rt.stack.running_web().await.is_none() {
        return Err(Error::Other("start the web server first".into()).into());
    }
    let https = cfg.general.auto_ssl && rt.stack.tls_error().is_none();
    let url = match (https, cfg.web.https_port, cfg.web.http_port) {
        (true, 443, _) => "https://localhost/phpmyadmin/".to_owned(),
        (true, port, _) => format!("https://localhost:{port}/phpmyadmin/"),
        (false, _, 80) => "http://localhost/phpmyadmin/".to_owned(),
        (false, _, port) => format!("http://localhost:{port}/phpmyadmin/"),
    };
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| Error::Other(e.to_string()))?;
    Ok(())
}
