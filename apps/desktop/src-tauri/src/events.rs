//! Events emitted to the UI. Names must match apps/desktop/src/lib/events.ts.

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::state::Runtime;

/// A service changed state; the UI refetches its status.
pub const SERVICE_STATUS: &str = "service-status";
/// config.toml was saved; the UI refetches settings.
pub const CONFIG_CHANGED: &str = "config-changed";
/// Sites or the hosts file state changed; the UI refetches the Sites page.
pub const SITES_CHANGED: &str = "sites-changed";
/// An install advanced: `{ id, version, stage, received, total }`.
pub const PACKAGE_PROGRESS: &str = "package-progress";
/// A profile switch started from the tray failed; payload: the message.
pub const PROFILE_SWITCH_FAILED: &str = "profile-switch-failed";
/// Installed versions or install jobs changed; the UI refetches Quick Add.
pub const PACKAGES_CHANGED: &str = "packages-changed";
/// A QuickApp job started, finished or was dismissed; the UI refetches it.
pub const QUICKAPP_CHANGED: &str = "quickapp-changed";
/// New QuickApp output: `{ jobId, lines, step, download }`.
pub const QUICKAPP_OUTPUT: &str = "quickapp-output";

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
            crate::tray::refresh(app.clone());
            // A web server came up (start, restart, Apache's restart-reload):
            // make sure the hosts file has its sites.
            if let E::Started { id, .. } = &event
                && (id == lumina_services::nginx::PROCESS_ID
                    || id == lumina_services::apache::PROCESS_ID)
            {
                crate::commands::sites::resync_later(&app);
            }
            // PHP (re)started, possibly on another version: the PATH shims
            // (php, composer) follow.
            if let E::Started { id, .. } = &event
                && id == lumina_services::php_fcgi::PROCESS_ID
            {
                crate::commands::env_path::refresh_shims_later(&app);
            }
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
