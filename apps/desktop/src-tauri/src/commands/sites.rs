//! Automatic virtual hosts: the Sites page, plus the glue that keeps vhost
//! files and the hosts file in line with the projects folder.
//!
//! [`resync`] is the single path for every trigger (folder watcher, web server
//! start, settings change, "Sync now"): one reload of the running web server
//! if a vhost file changed, then one administrator prompt if the hosts block
//! differs. Hosts are only touched while a web server runs (or when the user
//! asks), so opening Lumina never pops up a prompt by itself.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use lumina_core::Error;
use lumina_vhost::sync::{self, Attempt, HostsReport};
use lumina_vhost::watcher::{self, Watcher};
use lumina_vhost::{Scan, Site};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::error::CommandResult;
use crate::events;
use crate::state::{AppState, Runtime};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SitesOverview {
    #[serde(flatten)]
    pub scan: Scan,
    pub www_root: PathBuf,
    pub tld: String,
    pub auto_vhosts: bool,
    /// The web server serving the sites, if one runs.
    pub web_running: bool,
    pub http_port: u16,
    pub hosts: HostsReport,
}

fn overview(rt: &Runtime, web_running: bool) -> SitesOverview {
    let cfg = rt.config();
    let scan = rt.stack.sites(&cfg);
    let hosts = rt.hosts.report(&sync::desired_entries(&scan.sites));
    SitesOverview {
        www_root: rt.stack.www_root(&cfg),
        tld: cfg.general.tld.clone(),
        auto_vhosts: cfg.general.auto_vhosts,
        web_running,
        http_port: cfg.web.http_port,
        hosts,
        scan,
    }
}

#[tauri::command]
pub async fn sites_list(state: State<'_, AppState>) -> CommandResult<SitesOverview> {
    let rt = state.runtime()?;
    let running = rt.stack.running_web().await.is_some();
    Ok(overview(&rt, running))
}

/// "Sync now" / "Update hosts file": like an automatic sync, but asks for
/// administrator rights again even if the last prompt was declined.
#[tauri::command]
pub async fn sites_sync(app: AppHandle) -> CommandResult<Option<Attempt>> {
    Ok(resync(&app, true).await?)
}

/// Opens http://<domain>/ (with the port if it is not 80).
#[tauri::command]
pub fn site_open(app: AppHandle, state: State<'_, AppState>, domain: String) -> CommandResult<()> {
    let rt = state.runtime()?;
    let cfg = rt.config();
    let site = find(&rt, |s| s.domain == domain)?;
    let url = match cfg.web.http_port {
        80 => format!("http://{}/", site.domain),
        port => format!("http://{}:{port}/", site.domain),
    };
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| Error::Other(e.to_string()))?;
    Ok(())
}

/// Opens a project folder in the file manager.
#[tauri::command]
pub fn site_open_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> CommandResult<()> {
    let rt = state.runtime()?;
    let site = find(&rt, |s| s.name == name)?;
    app.opener()
        .open_path(site.path.to_string_lossy(), None::<&str>)
        .map_err(|e| Error::Other(e.to_string()))?;
    Ok(())
}

fn find(rt: &Runtime, pred: impl Fn(&Site) -> bool) -> CommandResult<Site> {
    rt.stack
        .sites(&rt.config())
        .sites
        .into_iter()
        .find(pred)
        .ok_or_else(|| Error::Other("no such site".into()).into())
}

/// Applies the projects folder to the running web server and the hosts file.
/// Returns the hosts attempt, or `None` when no web server runs and `force`
/// is off (the hosts file is then brought up to date when one starts).
pub async fn resync(app: &AppHandle, force: bool) -> lumina_core::Result<Option<Attempt>> {
    let rt = app.state::<AppState>().runtime()?;
    let cfg = rt.config();
    let reloaded = rt.stack.sync_sites(&cfg).await;
    let _ = app.emit(events::SITES_CHANGED, ());
    let running = match &reloaded {
        Ok(outcome) => outcome.is_some(),
        Err(err) => {
            log::error!("could not apply the sites to the web server: {err}");
            true
        }
    };
    if !running && !force {
        return reloaded.map(|_| None);
    }

    let desired = sync::desired_entries(&rt.stack.sites(&cfg).sites);
    let hosts = rt.clone();
    let notify = app.clone();
    // Let the UI show "waiting for approval" while the prompt is open.
    let pending = tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        let _ = notify.emit(events::SITES_CHANGED, ());
    });
    let attempt = hosts.hosts.ensure(desired, force).await;
    pending.abort();
    let _ = app.emit(events::SITES_CHANGED, ());
    reloaded?;
    Ok(Some(attempt))
}

/// [`resync`] in the background, logging failures.
pub fn resync_later(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(err) = resync(&app, false).await {
            log::error!("site sync failed: {err}");
        }
    });
}

/// (Re)starts the projects-folder watcher if the folder changed. Called at
/// start-up and after settings are saved.
pub fn watch(app: &AppHandle, rt: &Arc<Runtime>) {
    let www = rt.stack.www_root(&rt.config());
    let mut slot = rt.watcher.lock().unwrap_or_else(|e| e.into_inner());
    if slot.as_ref().is_some_and(|(dir, _)| *dir == www) {
        return;
    }
    // Drop the old watcher first so its thread stops.
    *slot = None;
    let handle = app.clone();
    let watcher = Watcher::spawn(www.clone(), watcher::POLL, move || {
        log::info!("projects folder changed");
        resync_later(&handle);
    });
    log::info!("watching {} for projects", www.display());
    *slot = Some((www, watcher));
}
