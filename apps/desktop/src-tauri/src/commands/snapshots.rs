//! Database snapshots on the Services page: save a database, restore it later.

use lumina_services::snapshots::{self, Snapshot};
use tauri::State;

use crate::error::CommandResult;
use crate::state::AppState;

#[tauri::command]
#[specta::specta]
pub fn snapshots_list(state: State<'_, AppState>) -> CommandResult<Vec<Snapshot>> {
    Ok(snapshots::list(state.runtime()?.stack.layout()))
}

/// The databases of the running `engine` (`mysql`, `mariadb`, `postgresql`).
#[tauri::command]
#[specta::specta]
pub async fn snapshot_databases(
    state: State<'_, AppState>,
    engine: String,
) -> CommandResult<Vec<String>> {
    let rt = state.runtime()?;
    let server = rt.stack.snapshot_server(&engine).await?;
    Ok(snapshots::databases(&server).await?)
}

#[tauri::command]
#[specta::specta]
pub async fn snapshot_create(
    state: State<'_, AppState>,
    engine: String,
    database: String,
) -> CommandResult<Snapshot> {
    let rt = state.runtime()?;
    let server = rt.stack.snapshot_server(&engine).await?;
    Ok(snapshots::create(rt.stack.layout(), &server, &database).await?)
}

/// Replaces the snapshot's database with its content (its server must run).
#[tauri::command]
#[specta::specta]
pub async fn snapshot_restore(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    let rt = state.runtime()?;
    let engine = id.split('/').next().unwrap_or_default().to_owned();
    let server = rt.stack.snapshot_server(&engine).await?;
    Ok(snapshots::restore(rt.stack.layout(), &server, &id).await?)
}

#[tauri::command]
#[specta::specta]
pub fn snapshot_delete(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    Ok(snapshots::delete(state.runtime()?.stack.layout(), &id)?)
}
