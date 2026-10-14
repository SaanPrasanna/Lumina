//! AppState managed by Tauri. Grows with each phase (config handle, orchestrator,
//! supervisor); Phase 0 only tracks the bootstrap file.

use std::path::PathBuf;
use std::sync::RwLock;

use lumina_config::bootstrap::Bootstrap;

pub struct AppState {
    /// `<app-config-dir>/bootstrap.toml`.
    pub bootstrap_path: PathBuf,
    /// `None` until the first-run wizard completes.
    pub bootstrap: RwLock<Option<Bootstrap>>,
}
