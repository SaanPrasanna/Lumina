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
            *state.runtime.write().unwrap_or_else(|e| e.into_inner()) = Some(runtime);
        }
        Err(err) => log::error!("cannot initialise data root: {err}"),
    }
}

/// `resources/templates`, bundled next to the app. In debug builds without
/// bundled resources, the repository copy is used.
fn templates_dir(app: &AppHandle) -> PathBuf {
    let bundled = app
        .path()
        .resource_dir()
        .map(|dir| dir.join("resources").join("templates"));
    match bundled {
        Ok(dir) if dir.is_dir() => dir,
        _ => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../resources/templates"),
    }
}
