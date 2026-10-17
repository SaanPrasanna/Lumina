//! App updates: signed packages from the GitHub releases.
//!
//! Every package is verified against the public key in tauri.conf.json before
//! it is installed; a package that does not match is refused. Installing
//! stops every service first (the installer replaces the program files, never
//! the data root) and remembers which ones ran, so the new version starts
//! them again.

use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use lumina_core::types::ServiceKind;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_updater::UpdaterExt;

use crate::error::CommandResult;
use crate::events;
use crate::state::{AppState, Runtime};

const STABLE: &str = "https://github.com/SaanPrasanna/Lumina/releases/latest/download/latest.json";
/// A rolling `beta` release holds the newest pre-release's `latest.json`.
const BETA: &str = "https://github.com/SaanPrasanna/Lumina/releases/download/beta/latest.json";
const FIRST_CHECK: Duration = Duration::from_secs(20);
const CHECK_EVERY: Duration = Duration::from_secs(24 * 60 * 60);
/// Services to start again after an update (in the data root's tmp folder).
const RESUME_FILE: &str = "resume-after-update.json";

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    /// Release notes (Markdown).
    pub notes: Option<String>,
    /// RFC 3339.
    pub date: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    pub current: String,
    pub channel: String,
    pub check: bool,
    pub available: Option<UpdateInfo>,
    /// Unix seconds of the last successful check.
    pub checked_at: Option<u64>,
    pub installing: bool,
    /// The last check or install failed.
    pub error: Option<String>,
}

/// Payload of `update-progress`.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgress {
    pub received: u64,
    pub total: Option<u64>,
}

#[derive(Default)]
pub struct Updates {
    state: Mutex<UpdateStatus>,
    /// The version a notification was shown for (once per version).
    notified: Mutex<Option<String>>,
}

impl Updates {
    fn lock(&self) -> std::sync::MutexGuard<'_, UpdateStatus> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Resume {
    from: String,
    services: Vec<ServiceKind>,
}

fn endpoint(channel: &str) -> &'static str {
    if channel == "beta" { BETA } else { STABLE }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn status(app: &AppHandle) -> UpdateStatus {
    let mut status = app.state::<Updates>().lock().clone();
    status.current = app.package_info().version.to_string();
    if let Ok(rt) = app.state::<AppState>().runtime() {
        let cfg = rt.config();
        status.channel = cfg.updates.channel;
        status.check = cfg.updates.check;
    } else {
        status.channel = "stable".into();
        status.check = true;
    }
    status
}

fn channel(app: &AppHandle) -> String {
    app.state::<AppState>()
        .runtime()
        .map_or_else(|_| "stable".into(), |rt| rt.config().updates.channel)
}

async fn find(app: &AppHandle) -> Result<Option<tauri_plugin_updater::Update>, String> {
    let url = tauri::Url::parse(endpoint(&channel(app))).map_err(|e| e.to_string())?;
    app.updater_builder()
        .endpoints(vec![url])
        .and_then(|b| b.build())
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())
}

/// Looks for a newer version and records the answer.
async fn check(app: &AppHandle) -> UpdateStatus {
    let result = find(app).await;
    {
        let updates = app.state::<Updates>();
        let mut state = updates.lock();
        match &result {
            Ok(found) => {
                state.available = found.as_ref().map(|u| UpdateInfo {
                    version: u.version.clone(),
                    notes: u.body.clone(),
                    date: u.date.map(|d| d.to_string()),
                });
                state.checked_at = Some(unix_now());
                state.error = None;
            }
            Err(err) => {
                log::warn!("update check failed: {err}");
                state.error = Some(err.clone());
            }
        }
    }
    let _ = app.emit(events::UPDATES_CHANGED, ());
    status(app)
}

/// Checks at start-up and once a day while `[updates] check` is on, and
/// notifies once per new version.
pub fn check_periodically(app: AppHandle) {
    if cfg!(debug_assertions) {
        // Development builds are not released; "Check now" still works.
        return;
    }
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK).await;
        loop {
            if status(&app).check {
                let found = check(&app).await.available;
                if let Some(info) = found {
                    let updates = app.state::<Updates>();
                    let mut notified = updates.notified.lock().unwrap_or_else(|e| e.into_inner());
                    if notified.as_deref() != Some(info.version.as_str()) {
                        log::info!("update available: {}", info.version);
                        crate::notify::send(
                            &app,
                            crate::notify::Note::UpdateAvailable {
                                version: &info.version,
                            },
                        );
                        *notified = Some(info.version);
                    }
                }
            }
            tokio::time::sleep(CHECK_EVERY).await;
        }
    });
}

#[tauri::command]
#[specta::specta]
pub fn updates_status(app: AppHandle) -> UpdateStatus {
    status(&app)
}

#[tauri::command]
#[specta::specta]
pub async fn updates_check(app: AppHandle) -> UpdateStatus {
    check(&app).await
}

/// Saves the channel and the automatic check (`[updates]`); a new channel
/// forgets what the old one offered.
#[tauri::command]
#[specta::specta]
pub fn updates_configure(
    app: AppHandle,
    state: State<'_, AppState>,
    channel: String,
    check: bool,
) -> CommandResult<UpdateStatus> {
    let rt = state.runtime()?;
    let mut cfg = rt.config();
    if !lumina_config::schema::UPDATE_CHANNELS.contains(&channel.as_str()) {
        return Err(
            lumina_core::Error::Config(format!("unknown update channel `{channel}`")).into(),
        );
    }
    if cfg.updates.channel != channel {
        app.state::<Updates>().lock().available = None;
    }
    cfg.updates.channel = channel;
    cfg.updates.check = check;
    lumina_config::save(&rt.layout.config_file(), &cfg)?;
    *rt.config.write().unwrap_or_else(|e| e.into_inner()) = cfg;
    let _ = app.emit(events::CONFIG_CHANGED, "updates");
    let _ = app.emit(events::UPDATES_CHANGED, ());
    Ok(status(&app))
}

/// The services that run now.
async fn running(rt: &Runtime) -> Vec<ServiceKind> {
    let mut out = Vec::new();
    let s = rt.stack.status(&rt.config()).await;
    if s.php.runtime.is_some() {
        out.push(ServiceKind::PhpFcgi);
    }
    if let Some(web) = rt.stack.running_web().await {
        out.push(web);
    }
    if s.database.runtime.is_some() {
        out.push(lumina_services::orchestrator::Orchestrator::configured_db(
            &rt.config(),
        ));
    }
    if s.postgresql.runtime.is_some() {
        out.push(ServiceKind::Postgresql);
    }
    if s.redis.runtime.is_some() {
        out.push(ServiceKind::Redis);
    }
    if s.mailpit.runtime.is_some() {
        out.push(ServiceKind::Mailpit);
    }
    out
}

/// Stops everything, downloads and verifies the update, installs it and
/// restarts Lumina; the services that ran are started again afterwards.
#[tauri::command]
#[specta::specta]
pub async fn updates_install(app: AppHandle) -> CommandResult<()> {
    {
        let updates = app.state::<Updates>();
        let mut state = updates.lock();
        if state.installing {
            return Err(
                lumina_core::Error::Other("an update is already being installed".into()).into(),
            );
        }
        state.installing = true;
        state.error = None;
    }
    let _ = app.emit(events::UPDATES_CHANGED, ());
    let result = install(&app).await;
    if let Err(err) = &result {
        log::error!("update failed: {err}");
        let updates = app.state::<Updates>();
        let mut state = updates.lock();
        state.installing = false;
        state.error = Some(err.clone());
    }
    let _ = app.emit(events::UPDATES_CHANGED, ());
    result.map_err(|e| lumina_core::Error::Other(e).into())
}

async fn install(app: &AppHandle) -> Result<(), String> {
    let update = find(app).await?.ok_or("Lumina is up to date")?;
    log::info!("installing Lumina {}", update.version);

    // Downloaded and verified first: a broken or tampered package never stops anything.
    let mut received = 0u64;
    let bytes = update
        .download(
            |chunk, total| {
                received += chunk as u64;
                let _ = app.emit(events::UPDATE_PROGRESS, UpdateProgress { received, total });
            },
            || log::info!("update downloaded and verified"),
        )
        .await
        .map_err(|e| e.to_string())?;

    if let Ok(rt) = app.state::<AppState>().runtime() {
        let services = running(&rt).await;
        let resume = Resume {
            from: app.package_info().version.to_string(),
            services,
        };
        let path = rt.layout.tmp().join(RESUME_FILE);
        if let Ok(json) = serde_json::to_vec(&resume) {
            let _ = std::fs::write(&path, json);
        }
        log::info!("stopping services for the update: {:?}", resume.services);
        let _ = lumina_tunnel::TunnelProvider::shutdown(&rt.tunnels).await;
        for (kind, err) in rt.stack.stop_all().await {
            log::error!("could not stop {kind:?} before the update: {err}");
        }
    }

    // Windows: the installer takes over and Lumina exits here.
    update.install(bytes).map_err(|e| e.to_string())?;
    app.restart();
}

/// After an update: starts the services that ran before it.
pub fn resume_services(app: &AppHandle) {
    let Ok(rt) = app.state::<AppState>().runtime() else {
        return;
    };
    let path = rt.layout.tmp().join(RESUME_FILE);
    let Some(resume) = std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice::<Resume>(&b).ok())
    else {
        return;
    };
    let _ = std::fs::remove_file(&path);
    log::info!(
        "updated from {} to {}; starting {:?} again",
        resume.from,
        app.package_info().version,
        resume.services
    );
    tauri::async_runtime::spawn(async move {
        let cfg = rt.config();
        for kind in resume.services {
            if let Err(err) = rt.stack.start(kind, &cfg).await {
                log::error!("could not start {kind:?} after the update: {err}");
            }
        }
    });
}
