//! App startup: load the bootstrap file and, if a data root is configured,
//! build the runtime (config, leftover-process sweep, supervisor, services).

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use lumina_config::bootstrap;
use lumina_core::paths::Layout;
use tauri::{AppHandle, Manager};

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

    app.manage(AppState {
        bootstrap_path,
        bootstrap: RwLock::new(bootstrap.clone()),
        templates,
        runtime: RwLock::new(None),
    });

    match bootstrap {
        Some(b) => {
            log::info!("data root: {}", b.data_root.display());
            start_runtime(app.handle(), Layout::new(&b.data_root));
        }
        None => log::info!("no bootstrap file; first-run wizard will be shown"),
    }
    Ok(())
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
