//! Events emitted to the UI. Names must match apps/desktop/src/lib/events.ts.

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::state::Runtime;

/// A service changed state; the UI refetches its status.
pub const SERVICE_STATUS: &str = "service-status";
/// config.toml was saved; the UI refetches settings.
pub const CONFIG_CHANGED: &str = "config-changed";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceStatusEvent {
    /// Supervisor process ID, e.g. `php-fcgi`.
    pub id: String,
    /// started | exited | crashed | restarting | failed
    pub kind: &'static str,
    pub detail: Option<String>,
}

/// Forwards supervisor lifecycle events to the UI for as long as the app runs.
pub fn bridge_supervisor(app: AppHandle, runtime: &Arc<Runtime>) {
    let mut rx = runtime.supervisor.subscribe();
    tauri::async_runtime::spawn(async move {
        use lumina_process::ProcessEvent as E;
        use tokio::sync::broadcast::error::RecvError;
        loop {
            let event = match rx.recv().await {
                Ok(event) => event,
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            };
            let payload = match &event {
                E::Started { id, pid } => (id, "started", Some(format!("pid {pid}"))),
                E::Exited {
                    id, expected: true, ..
                } => (id, "exited", None),
                E::Exited { id, code, .. } => (id, "crashed", Some(format!("code {code:?}"))),
                E::Restarting { id, attempt } => {
                    (id, "restarting", Some(format!("attempt {attempt}")))
                }
                E::Failed { id, reason } => (id, "failed", Some(reason.clone())),
            };
            let _ = app.emit(
                SERVICE_STATUS,
                ServiceStatusEvent {
                    id: payload.0.clone(),
                    kind: payload.1,
                    detail: payload.2,
                },
            );
        }
    });
}
