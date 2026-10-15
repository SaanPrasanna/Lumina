//! Read/write config.toml sections.

use std::path::PathBuf;

use lumina_config::{AppConfig, ConfigSource, Warning};
use lumina_core::Error;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::error::CommandResult;
use crate::events;
use crate::state::{AppState, Runtime};

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub config: AppConfig,
    pub warnings: Vec<Warning>,
    pub source: ConfigSource,
    pub path: PathBuf,
}

fn snapshot(runtime: &Runtime) -> Settings {
    Settings {
        config: runtime.config(),
        warnings: runtime
            .warnings
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone(),
        source: runtime.config_source,
        path: runtime.layout.config_file(),
    }
}

#[tauri::command]
#[specta::specta]
pub fn settings_get(state: State<'_, AppState>) -> CommandResult<Settings> {
    let runtime = state.runtime()?;
    Ok(snapshot(&runtime))
}

/// Replaces one section (e.g. `"php"`, `"editor"`) with `value`, validates the
/// result (repairing what it can), saves config.toml and notifies the UI.
/// Missing fields in `value` take their defaults.
/// One config section from the UI, any shape: it is checked against the schema
/// when merged (`unknown` in TypeScript; serde_json::Value is recursive).
#[derive(Debug, serde::Deserialize, specta::Type)]
#[serde(transparent)]
pub struct SectionValue(#[specta(type = specta_typescript::Unknown)] serde_json::Value);

#[tauri::command]
#[specta::specta]
pub fn settings_update(
    app: AppHandle,
    state: State<'_, AppState>,
    section: String,
    value: SectionValue,
) -> CommandResult<Settings> {
    let value = value.0;
    let runtime = state.runtime()?;
    let bad = |e: serde_json::Error| Error::Config(e.to_string());

    let mut json = serde_json::to_value(runtime.config()).map_err(bad)?;
    let Some(slot) = json.get_mut(&section).filter(|v| v.is_object()) else {
        return Err(Error::Config(format!("unknown settings section `{section}`")).into());
    };
    *slot = value;
    let mut config: AppConfig = serde_json::from_value(json).map_err(bad)?;

    let warnings = lumina_config::validate::validate(&mut config);
    lumina_config::save(&runtime.layout.config_file(), &config)?;
    log::info!("settings section `{section}` saved");

    *runtime.config.write().unwrap_or_else(|e| e.into_inner()) = config;
    *runtime.warnings.write().unwrap_or_else(|e| e.into_inner()) = warnings;
    let _ = app.emit(events::CONFIG_CHANGED, &section);
    if matches!(section.as_str(), "php" | "database" | "tools") {
        crate::commands::env_path::refresh_shims_later(&app);
    }
    if section == "general" || section == "sites" {
        // The projects folder, suffix or a site's settings may have changed.
        crate::commands::sites::watch(&app, &runtime);
        crate::commands::sites::resync_later(&app);
    }
    Ok(snapshot(&runtime))
}
