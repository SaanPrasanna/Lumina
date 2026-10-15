//! App-level commands: version and the first-run (bootstrap) wizard.

use std::path::PathBuf;

use lumina_config::bootstrap::{self, Bootstrap};
use lumina_core::constants::DEFAULT_LANGUAGE;
use lumina_core::paths::{Layout, default_data_root};
use serde::Serialize;
use tauri::State;

use crate::error::CommandResult;
use crate::state::AppState;

#[tauri::command]
#[specta::specta]
pub fn app_version(app: tauri::AppHandle) -> String {
    app.package_info().version.to_string()
}

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapStatus {
    /// `false` → show the first-run wizard.
    pub configured: bool,
    pub data_root: Option<PathBuf>,
    /// Suggestion for the wizard's folder field.
    pub default_data_root: PathBuf,
    pub language: String,
}

fn status(bootstrap: Option<&Bootstrap>) -> BootstrapStatus {
    BootstrapStatus {
        configured: bootstrap.is_some(),
        data_root: bootstrap.map(|b| b.data_root.clone()),
        default_data_root: default_data_root(),
        language: bootstrap.map_or_else(|| DEFAULT_LANGUAGE.to_owned(), |b| b.language.clone()),
    }
}

#[tauri::command]
#[specta::specta]
pub fn bootstrap_status(state: State<'_, AppState>) -> BootstrapStatus {
    let guard = state.bootstrap.read().unwrap_or_else(|e| e.into_inner());
    status(guard.as_ref())
}

/// Finishes the first-run wizard: validates input, creates the data root layout,
/// then persists the bootstrap file (last, so a failure leaves the wizard rerunnable).
#[tauri::command]
#[specta::specta]
pub fn bootstrap_complete(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    data_root: PathBuf,
    language: String,
) -> CommandResult<BootstrapStatus> {
    let mut bootstrap = Bootstrap::new(data_root, language)?;
    // A fresh data root has no older config to keep.
    bootstrap.last_version = app.package_info().version.to_string();
    Layout::new(&bootstrap.data_root).ensure()?;
    bootstrap::save(&state.bootstrap_path, &bootstrap)?;
    log::info!(
        "first run complete; data root: {}",
        bootstrap.data_root.display()
    );

    let layout = Layout::new(&bootstrap.data_root);
    let status = {
        let mut guard = state.bootstrap.write().unwrap_or_else(|e| e.into_inner());
        *guard = Some(bootstrap);
        status(guard.as_ref())
    };
    crate::setup::start_runtime(&app, layout);
    Ok(status)
}

/// Persists a language change from the UI language switcher.
#[tauri::command]
#[specta::specta]
pub fn set_language(state: State<'_, AppState>, language: String) -> CommandResult<()> {
    let mut guard = state.bootstrap.write().unwrap_or_else(|e| e.into_inner());
    // Before first run there is nothing to persist; the wizard sends the language.
    let Some(current) = guard.as_ref() else {
        return Ok(());
    };
    let updated = Bootstrap::new(current.data_root.clone(), language)?;
    bootstrap::save(&state.bootstrap_path, &updated)?;
    *guard = Some(updated);
    Ok(())
}
