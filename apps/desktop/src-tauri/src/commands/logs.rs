//! The Logs page: every log Lumina writes, read incrementally for a live tail.
//!
//! Logs are named by an ID (`nginx_error.log`, `quickapp/blog.log`, `app`),
//! never by a path from the UI: only files Lumina lists can be read.

use std::path::PathBuf;

use lumina_process::logs::{self, Chunk};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::error::CommandResult;
use crate::state::AppState;

/// The app's own log (`lumina.log` in the OS log folder).
const APP_LOG: &str = "app";
/// At most this much is sent per read (the first read shows the end of the file).
const MAX_READ: u64 = 512 * 1024;

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LogSource {
    pub id: String,
    /// `service`, `quickapp` or `app`.
    pub group: &'static str,
    pub path: PathBuf,
    pub size: u64,
    /// Unix seconds of the last change.
    pub modified: u64,
}

fn source(id: String, group: &'static str, path: PathBuf) -> Option<LogSource> {
    let meta = std::fs::metadata(&path).ok().filter(|m| m.is_file())?;
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Some(LogSource {
        id,
        group,
        path,
        size: meta.len(),
        modified,
    })
}

/// `*.log` directly in `dir` (rotated `.log.1` copies are left out).
fn logs_in(dir: &std::path::Path) -> Vec<(String, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<_> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_owned();
            name.ends_with(".log").then(|| (name, e.path()))
        })
        .collect();
    out.sort();
    out
}

fn sources(app: &AppHandle, state: &AppState) -> Vec<LogSource> {
    let mut out = Vec::new();
    if let Ok(dir) = app.path().app_log_dir() {
        out.extend(source(APP_LOG.into(), "app", dir.join("lumina.log")));
    }
    if let Ok(rt) = state.runtime() {
        let dir = rt.layout.logs();
        for (name, path) in logs_in(&dir) {
            out.extend(source(name, "service", path));
        }
        for (name, path) in logs_in(&dir.join("quickapp")) {
            out.extend(source(format!("quickapp/{name}"), "quickapp", path));
        }
    }
    out
}

fn find(app: &AppHandle, state: &AppState, id: &str) -> CommandResult<LogSource> {
    sources(app, state)
        .into_iter()
        .find(|s| s.id == id)
        .ok_or_else(|| lumina_core::Error::Other(format!("no log named `{id}`")).into())
}

#[tauri::command]
#[specta::specta]
pub fn logs_list(app: AppHandle, state: State<'_, AppState>) -> Vec<LogSource> {
    sources(&app, &state)
}

/// What `id` gained since `offset` (`None`: its last part, to start a tail).
#[tauri::command]
#[specta::specta]
pub fn logs_read(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    offset: Option<u64>,
) -> CommandResult<Chunk> {
    let log = find(&app, &state, &id)?;
    Ok(logs::read_from(&log.path, offset, MAX_READ))
}

/// Opens a log in the system's default viewer.
#[tauri::command]
#[specta::specta]
pub fn logs_open(app: AppHandle, state: State<'_, AppState>, id: String) -> CommandResult<()> {
    let log = find(&app, &state, &id)?;
    app.opener()
        .open_path(log.path.to_string_lossy(), None::<&str>)
        .map_err(|e| lumina_core::Error::Other(e.to_string()))?;
    Ok(())
}

/// Opens the folder with the service logs.
#[tauri::command]
#[specta::specta]
pub fn logs_open_folder(app: AppHandle, state: State<'_, AppState>) -> CommandResult<()> {
    let dir = state.runtime()?.layout.logs();
    app.opener()
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(|e| lumina_core::Error::Other(e.to_string()))?;
    Ok(())
}
