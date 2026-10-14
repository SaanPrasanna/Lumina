//! PHP FastCGI service commands. Phase 2 generalises these into
//! `service_*(kind)` once the orchestrator manages several services.

use lumina_core::Error;
use lumina_services::php_fcgi::{PhpInstall, PhpRuntime, PhpStatus};
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_opener::OpenerExt;

use crate::error::CommandResult;
use crate::events;
use crate::state::AppState;

#[tauri::command]
pub async fn php_status(state: State<'_, AppState>) -> CommandResult<PhpStatus> {
    Ok(state.runtime()?.php.status().await)
}

#[tauri::command]
pub async fn php_versions(state: State<'_, AppState>) -> CommandResult<Vec<PhpInstall>> {
    Ok(state.runtime()?.php.versions().await)
}

#[tauri::command]
pub async fn php_start(state: State<'_, AppState>) -> CommandResult<PhpRuntime> {
    let runtime = state.runtime()?;
    let config = runtime.config().php;
    Ok(runtime.php.start(&config).await?)
}

#[tauri::command]
pub async fn php_stop(state: State<'_, AppState>) -> CommandResult<()> {
    Ok(state.runtime()?.php.stop().await?)
}

#[tauri::command]
pub async fn php_restart(state: State<'_, AppState>) -> CommandResult<PhpRuntime> {
    let runtime = state.runtime()?;
    let config = runtime.config().php;
    Ok(runtime.php.restart(&config).await?)
}

/// Saves `version` ("" = newest) as the PHP version and, if PHP is running,
/// switches to it: the old pool is stopped and verified gone before the new
/// one starts.
#[tauri::command]
pub async fn php_set_version(
    app: AppHandle,
    state: State<'_, AppState>,
    version: String,
) -> CommandResult<PhpStatus> {
    let runtime = state.runtime()?;
    let mut config = runtime.config();
    config.php.version = version;
    lumina_config::save(&runtime.layout.config_file(), &config)?;
    let php_config = config.php.clone();
    *runtime.config.write().unwrap_or_else(|e| e.into_inner()) = config;
    let _ = app.emit(events::CONFIG_CHANGED, "php");

    let running = runtime.php.status().await.runtime.is_some();
    if running {
        runtime.php.restart(&php_config).await?;
    }
    Ok(runtime.php.status().await)
}

/// Opens the PHP log in the system's default viewer.
#[tauri::command]
pub async fn php_open_log(app: AppHandle, state: State<'_, AppState>) -> CommandResult<()> {
    let log = state.runtime()?.php.log_file();
    if !log.exists() {
        return Err(Error::Other(format!("{} does not exist yet", log.display())).into());
    }
    app.opener()
        .open_path(log.to_string_lossy(), None::<&str>)
        .map_err(|e| Error::Other(e.to_string()))?;
    Ok(())
}
