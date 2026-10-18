//! The webhook catcher on the Sites page: start, stop, list, replay.

use lumina_services::webhooks::WebhooksStatus;
use tauri::State;

use crate::error::CommandResult;
use crate::state::AppState;

#[tauri::command]
#[specta::specta]
pub async fn webhooks_status(state: State<'_, AppState>) -> CommandResult<WebhooksStatus> {
    Ok(state.runtime()?.stack.webhooks().status().await)
}

/// Starts (or restarts) the catcher with the saved `[webhooks]` settings.
#[tauri::command]
#[specta::specta]
pub async fn webhooks_start(state: State<'_, AppState>) -> CommandResult<()> {
    let rt = state.runtime()?;
    Ok(rt.stack.start_webhooks(&rt.config()).await?)
}

#[tauri::command]
#[specta::specta]
pub async fn webhooks_stop(state: State<'_, AppState>) -> CommandResult<()> {
    state.runtime()?.stack.webhooks().stop().await;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn webhooks_clear(state: State<'_, AppState>) -> CommandResult<()> {
    state.runtime()?.stack.webhooks().clear().await;
    Ok(())
}

/// Sends recorded webhook `id` to the site of folder `site`; returns the
/// site's HTTP status.
#[tauri::command]
#[specta::specta]
pub async fn webhook_replay(
    state: State<'_, AppState>,
    id: u32,
    site: String,
) -> CommandResult<u16> {
    let rt = state.runtime()?;
    let target = rt.stack.webhook_target(&rt.config(), &site)?;
    Ok(rt.stack.webhooks().replay(u64::from(id), &target).await?)
}
