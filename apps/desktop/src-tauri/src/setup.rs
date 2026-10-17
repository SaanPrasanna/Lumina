//! App startup: load the bootstrap file and, if a data root is configured,
//! build the runtime (config, leftover-process sweep, supervisor, services).

use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use lumina_config::bootstrap;
use lumina_core::paths::Layout;
use lumina_core::types::ServiceKind;
use tauri::{AppHandle, Emitter, Manager};

use crate::events;
use crate::state::{AppState, Runtime};

pub fn init(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let bootstrap_path = app.path().app_config_dir()?.join(bootstrap::FILE_NAME);
    let templates = templates_dir(app.handle());
    log::info!("templates: {}", templates.display());

    let bootstrap = match bootstrap::load(&bootstrap_path) {
        Ok(found) => found,
        Err(err) => {
            // A corrupt bootstrap file must not brick the app: rerun the wizard.
            log::error!("ignoring unreadable bootstrap file: {err}");
            None
        }
    };

    app.manage(crate::commands::packages::Jobs::default());
    app.manage(crate::commands::quickapp::Jobs::default());
    app.manage(crate::commands::updates::Updates::default());
    app.manage(AppState {
        bootstrap_path,
        bootstrap: RwLock::new(bootstrap.clone()),
        templates,
        runtime: RwLock::new(None),
    });

    match bootstrap {
        Some(mut b) => {
            log::info!("data root: {}", b.data_root.display());
            let version = app.package_info().version.to_string();
            if b.last_version != version {
                // A new (or older) version: keep the config as it was before
                // this version reads and maybe migrates it.
                let config = Layout::new(&b.data_root).config_file();
                match lumina_config::backup_before_upgrade(&config, &version) {
                    Ok(Some(copy)) => log::info!(
                        "first start of {version} (last: {}): config saved as {}",
                        if b.last_version.is_empty() {
                            "unknown"
                        } else {
                            &b.last_version
                        },
                        copy.display()
                    ),
                    Ok(None) => {}
                    Err(err) => log::error!("could not back up config.toml: {err}"),
                }
                b.last_version = version;
                let path = app.state::<AppState>().bootstrap_path.clone();
                if let Err(err) = bootstrap::save(&path, &b) {
                    log::error!("could not record the version: {err}");
                }
                *app.state::<AppState>()
                    .bootstrap
                    .write()
                    .unwrap_or_else(|e| e.into_inner()) = Some(b.clone());
            }
            start_runtime(app.handle(), Layout::new(&b.data_root));
        }
        None => log::info!("no bootstrap file; first-run wizard will be shown"),
    }
    watch_for_wake(app.handle().clone());
    Ok(())
}

/// How long a service gets to accept a connection after a wake.
const WAKE_PROBE: Duration = Duration::from_secs(5);
/// Lets the network stack come back before the services are probed.
const WAKE_SETTLE: Duration = Duration::from_secs(3);

/// After the computer sleeps, checks the running services and restarts any
/// that no longer answer, renews certificates that came due while it slept,
/// and has the UI refetch what may have changed (tunnels drop during sleep).
fn watch_for_wake(app: AppHandle) {
    tauri::async_runtime::spawn(lumina_process::wake::watch(move |asleep| {
        log::info!(
            "the computer woke up after about {} s; checking services",
            asleep.as_secs()
        );
        tauri::async_runtime::spawn(after_wake(app.clone()));
    }));
}

async fn after_wake(app: AppHandle) {
    tokio::time::sleep(WAKE_SETTLE).await;
    let Ok(runtime) = app.state::<AppState>().runtime() else {
        return; // first-run wizard: nothing runs yet
    };
    let cfg = runtime.config();
    for (kind, result) in runtime.stack.heal(&cfg, WAKE_PROBE).await {
        match result {
            Ok(()) => log::warn!("restarted {kind:?}: it no longer answered after sleep"),
            Err(err) => {
                log::error!("could not restart {kind:?} after sleep: {err}");
                let service = match kind {
                    ServiceKind::PhpFcgi => "PHP",
                    ServiceKind::Nginx => "nginx",
                    ServiceKind::Apache => "Apache",
                    ServiceKind::Mysql => "MySQL",
                    ServiceKind::Mariadb => "MariaDB",
                    ServiceKind::Redis => "Redis",
                    ServiceKind::Postgresql => "PostgreSQL",
                };
                crate::notify::send(&app, crate::notify::Note::Failed { service });
            }
        }
    }
    // The 12-hour renewal timer does not count time asleep on macOS and Linux.
    crate::commands::sites::resync_later(&app);
    crate::tray::refresh(app.clone());
    let _ = app.emit(events::TUNNELS_CHANGED, ());
}

/// Builds the runtime for `layout` and stores it in `AppState`. Errors are
/// logged; commands then report "not set up" until the problem is fixed.
pub fn start_runtime(app: &AppHandle, layout: Layout) {
    let state = app.state::<AppState>();
    match Runtime::init(layout, state.templates.clone()) {
        Ok(runtime) => {
            let runtime = Arc::new(runtime);
            events::bridge_supervisor(app.clone(), &runtime);
            crate::commands::sites::watch(app, &runtime);
            renew_certificates_periodically(app.clone());
            crate::commands::env_path::refresh_shims_later(app);
            *state.runtime.write().unwrap_or_else(|e| e.into_inner()) = Some(runtime);
        }
        Err(err) => log::error!("cannot initialise data root: {err}"),
    }
}

/// Site certificates are renewed whenever the web server's config is rendered
/// (they are replaced 30 days before they expire). For an app left running for
/// weeks, re-render twice a day; an unchanged config costs no reload.
fn renew_certificates_periodically(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(12 * 60 * 60)).await;
            crate::commands::sites::resync_later(&app);
        }
    });
}

/// Starts PHP and the web server if `general.autostart_services` is on.
pub fn autostart(app: &AppHandle) {
    let Ok(runtime) = app.state::<AppState>().runtime() else {
        return;
    };
    let cfg = runtime.config();
    if !cfg.general.autostart_services {
        return;
    }
    tauri::async_runtime::spawn(async move {
        log::info!("auto-starting services");
        if let Err(err) = runtime.stack.start_all(&cfg).await {
            log::error!("auto-start failed: {err}");
        }
    });
}

/// Whether closing the window should hide Lumina in the tray. Before the
/// first-run wizard completes, closing quits.
pub fn close_to_tray(app: &AppHandle) -> bool {
    app.state::<AppState>()
        .runtime()
        .is_ok_and(|rt| rt.config().general.close_to_tray)
}

/// `resources/templates`, bundled next to the app. In debug builds without
/// bundled resources, the repository copy is used.
fn templates_dir(app: &AppHandle) -> PathBuf {
    // Debug builds read the repository copy directly, so template edits apply
    // without a rebuild (the bundled copy is only refreshed by the build script).
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../resources/templates");
    if cfg!(debug_assertions) && repo.is_dir() {
        return repo;
    }
    let bundled = app
        .path()
        .resource_dir()
        .map(|dir| dir.join("resources").join("templates"));
    match bundled {
        Ok(dir) if dir.is_dir() => dir,
        _ => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../resources/templates"),
    }
}
