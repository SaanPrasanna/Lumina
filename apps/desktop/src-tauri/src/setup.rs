//! App startup: locate and load the bootstrap file, make sure the data root layout
//! exists, and register `AppState`. Later phases add config loading, the tray and
//! auto-start services here.

use std::sync::RwLock;

use lumina_config::bootstrap;
use lumina_core::paths::Layout;
use tauri::Manager;

use crate::state::AppState;

pub fn init(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let bootstrap_path = app.path().app_config_dir()?.join(bootstrap::FILE_NAME);

    let bootstrap = match bootstrap::load(&bootstrap_path) {
        Ok(found) => found,
        Err(err) => {
            // A corrupt bootstrap file must not brick the app: rerun the wizard.
            log::error!("ignoring unreadable bootstrap file: {err}");
            None
        }
    };

    match &bootstrap {
        Some(b) => {
            log::info!("data root: {}", b.data_root.display());
            // Recreates folders the user may have deleted; idempotent.
            if let Err(err) = Layout::new(&b.data_root).ensure() {
                log::error!("cannot prepare data root: {err}");
            }
        }
        None => log::info!("no bootstrap file; first-run wizard will be shown"),
    }

    app.manage(AppState {
        bootstrap_path,
        bootstrap: RwLock::new(bootstrap),
    });
    Ok(())
}
