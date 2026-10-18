//! Service commands. Everything goes through the orchestrator, which starts PHP
//! before the web server, stops in reverse, and serialises concurrent requests.

use lumina_config::schema::{DatabaseEngine, WebServer};
use lumina_core::Error;
use lumina_core::types::ServiceKind;
use lumina_services::install::Install;
use lumina_services::orchestrator::StackStatus;
use lumina_services::web::ReloadOutcome;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::error::CommandResult;
use crate::events;
use crate::state::AppState;

#[tauri::command]
#[specta::specta]
pub async fn services_status(state: State<'_, AppState>) -> CommandResult<StackStatus> {
    let rt = state.runtime()?;
    Ok(rt.stack.status(&rt.config()).await)
}

#[tauri::command]
#[specta::specta]
pub async fn service_start(state: State<'_, AppState>, kind: ServiceKind) -> CommandResult<()> {
    let rt = state.runtime()?;
    Ok(rt.stack.start(kind, &rt.config()).await?)
}

#[tauri::command]
#[specta::specta]
pub async fn service_stop(state: State<'_, AppState>, kind: ServiceKind) -> CommandResult<()> {
    Ok(state.runtime()?.stack.stop(kind).await?)
}

#[tauri::command]
#[specta::specta]
pub async fn service_restart(state: State<'_, AppState>, kind: ServiceKind) -> CommandResult<()> {
    let rt = state.runtime()?;
    Ok(rt.stack.restart(kind, &rt.config()).await?)
}

/// Applies a changed web-server config with a single confirmed reload.
#[tauri::command]
#[specta::specta]
pub async fn service_reload(
    state: State<'_, AppState>,
    kind: ServiceKind,
) -> CommandResult<ReloadOutcome> {
    let rt = state.runtime()?;
    Ok(rt.stack.reload(kind, &rt.config()).await?)
}

#[tauri::command]
#[specta::specta]
pub async fn services_start_all(state: State<'_, AppState>) -> CommandResult<()> {
    let rt = state.runtime()?;
    Ok(rt.stack.start_all(&rt.config()).await?)
}

#[tauri::command]
#[specta::specta]
pub async fn services_stop_all(state: State<'_, AppState>) -> CommandResult<()> {
    let failures = state.runtime()?.stack.stop_all().await;
    match failures.into_iter().next() {
        Some((_, err)) => Err(err.into()),
        None => Ok(()),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn php_versions(state: State<'_, AppState>) -> CommandResult<Vec<Install>> {
    Ok(state.runtime()?.stack.php().versions().await)
}

#[tauri::command]
#[specta::specta]
pub async fn web_versions(
    state: State<'_, AppState>,
    server: WebServer,
) -> CommandResult<Vec<Install>> {
    let rt = state.runtime()?;
    Ok(match server {
        WebServer::Nginx => rt.stack.nginx().versions().await,
        WebServer::Apache => rt.stack.apache().versions().await,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn db_versions(
    state: State<'_, AppState>,
    engine: DatabaseEngine,
) -> CommandResult<Vec<Install>> {
    Ok(state.runtime()?.stack.database(engine).versions().await)
}

#[tauri::command]
#[specta::specta]
pub async fn redis_versions(state: State<'_, AppState>) -> CommandResult<Vec<Install>> {
    Ok(state.runtime()?.stack.redis().versions().await)
}

#[tauri::command]
#[specta::specta]
pub async fn postgresql_versions(state: State<'_, AppState>) -> CommandResult<Vec<Install>> {
    Ok(state.runtime()?.stack.postgresql().versions().await)
}

/// MongoDB, Valkey, Meilisearch, Typesense and SeaweedFS (S3).
#[tauri::command]
#[specta::specta]
pub async fn extras_status(
    state: State<'_, AppState>,
) -> CommandResult<Vec<lumina_services::extra::ExtraStatus>> {
    let rt = state.runtime()?;
    let mut extras = rt.stack.extras_status(&rt.config()).await;
    // A service this system cannot get only shows once someone installed it.
    extras.retain(|e| {
        !e.versions.is_empty() || e.runtime.is_some() || super::packages::offered_here(&rt, &e.id)
    });
    Ok(extras)
}

#[tauri::command]
#[specta::specta]
pub async fn extra_start(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    let rt = state.runtime()?;
    Ok(rt.stack.start_extra(&id, &rt.config()).await?)
}

#[tauri::command]
#[specta::specta]
pub async fn extra_stop(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    Ok(state.runtime()?.stack.stop_extra(&id).await?)
}

#[tauri::command]
#[specta::specta]
pub async fn extra_restart(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    let rt = state.runtime()?;
    Ok(rt.stack.restart_extra(&id, &rt.config()).await?)
}

#[tauri::command]
#[specta::specta]
pub fn extra_open_log(app: AppHandle, state: State<'_, AppState>, id: String) -> CommandResult<()> {
    let rt = state.runtime()?;
    let def = lumina_services::extra::definition(&id)
        .ok_or_else(|| Error::Other(format!("no service `{id}`")))?;
    let log = rt.stack.extras().log_file(def);
    if !log.exists() {
        return Err(Error::Other(format!("{} does not exist yet", log.display())).into());
    }
    open_path(&app, &log.to_string_lossy())
}

/// Opens a service's web page (Meilisearch's dashboard).
#[tauri::command]
#[specta::specta]
pub fn extra_open_ui(app: AppHandle, state: State<'_, AppState>, id: String) -> CommandResult<()> {
    let rt = state.runtime()?;
    let def = lumina_services::extra::definition(&id)
        .ok_or_else(|| Error::Other(format!("no service `{id}`")))?;
    let Some(url) = def.web_ui else {
        return Err(Error::Other(format!("{} has no web page", def.name)).into());
    };
    let port = lumina_services::extra::settings(&rt.config().services, def).port;
    app.opener()
        .open_url(url.replace("{port}", &port.to_string()), None::<&str>)
        .map_err(|e| Error::Other(e.to_string()))?;
    Ok(())
}

/// Opens http://localhost/adminer/ in the default browser.
#[tauri::command]
#[specta::specta]
pub async fn adminer_open(app: AppHandle, state: State<'_, AppState>) -> CommandResult<()> {
    let rt = state.runtime()?;
    let cfg = rt.config();
    if rt.packages.installed("adminer").is_empty() {
        return Err(Error::not_installed("adminer", "Adminer").into());
    }
    if rt.stack.running_web().await.is_none() {
        return Err(Error::Other("start the web server first".into()).into());
    }
    let https = cfg.general.auto_ssl && rt.stack.tls_error().is_none();
    let url = match (https, cfg.web.https_port, cfg.web.http_port) {
        (true, 443, _) => "https://localhost/adminer/".to_owned(),
        (true, port, _) => format!("https://localhost:{port}/adminer/"),
        (false, _, 80) => "http://localhost/adminer/".to_owned(),
        (false, _, port) => format!("http://localhost:{port}/adminer/"),
    };
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| Error::Other(e.to_string()))?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn mailpit_versions(state: State<'_, AppState>) -> CommandResult<Vec<Install>> {
    Ok(state.runtime()?.stack.mailpit().versions().await)
}

/// Opens Mailpit's inbox in the default browser.
#[tauri::command]
#[specta::specta]
pub fn mailpit_open(app: AppHandle) -> CommandResult<()> {
    mailpit_open_for(&app)
}

/// Shared with the tray menu.
pub fn mailpit_open_for(app: &AppHandle) -> CommandResult<()> {
    let port = app.state::<AppState>().runtime()?.config().mailpit.ui_port;
    app.opener()
        .open_url(format!("http://127.0.0.1:{port}/"), None::<&str>)
        .map_err(|e| Error::Other(e.to_string()))?;
    Ok(())
}

/// Saves `version` ("" = newest) as the PHP version and, if PHP is running,
/// switches to it: the old pool is stopped and verified gone before the new
/// one starts, and a running web server is pointed at the new pool.
#[tauri::command]
#[specta::specta]
pub async fn php_set_version(
    app: AppHandle,
    state: State<'_, AppState>,
    version: String,
) -> CommandResult<()> {
    let rt = state.runtime()?;
    let mut config = rt.config();
    config.php.version = version;
    lumina_config::save(&rt.layout.config_file(), &config)?;
    *rt.config.write().unwrap_or_else(|e| e.into_inner()) = config.clone();
    let _ = app.emit(events::CONFIG_CHANGED, "php");

    if rt.stack.php().status().await.runtime.is_some() {
        rt.stack.restart(ServiceKind::PhpFcgi, &config).await?;
    }
    Ok(())
}

/// Opens a service's log in the system's default viewer.
#[tauri::command]
#[specta::specta]
pub async fn service_open_log(
    app: AppHandle,
    state: State<'_, AppState>,
    kind: ServiceKind,
) -> CommandResult<()> {
    open_log_for(&app, &*state.runtime()?, kind)
}

/// [`service_open_log`], shared with the tray.
pub fn open_log_for(
    app: &AppHandle,
    rt: &crate::state::Runtime,
    kind: ServiceKind,
) -> CommandResult<()> {
    let log = match kind {
        ServiceKind::PhpFcgi => rt.stack.php().log_file(),
        ServiceKind::Nginx => rt.stack.nginx().log_file(),
        ServiceKind::Apache => rt.stack.apache().log_file(),
        ServiceKind::Mysql => rt.stack.database(DatabaseEngine::Mysql).log_file(),
        ServiceKind::Mariadb => rt.stack.database(DatabaseEngine::Mariadb).log_file(),
        ServiceKind::Redis => rt.stack.redis().log_file(),
        ServiceKind::Postgresql => rt.stack.postgresql().log_file(),
        ServiceKind::Mailpit => rt.stack.mailpit().log_file(),
    };
    if !log.exists() {
        return Err(Error::Other(format!("{} does not exist yet", log.display())).into());
    }
    open_path(app, &log.to_string_lossy())
}

/// Opens http://localhost (with the configured port) in the default browser.
#[tauri::command]
#[specta::specta]
pub fn open_localhost(app: AppHandle) -> CommandResult<()> {
    open_localhost_for(&app)
}

/// Opens the projects folder in the file manager.
#[tauri::command]
#[specta::specta]
pub fn open_www(app: AppHandle) -> CommandResult<()> {
    open_www_for(&app)
}

/// Shared with the tray menu.
pub fn open_localhost_for(app: &AppHandle) -> CommandResult<()> {
    let port = app.state::<AppState>().runtime()?.config().web.http_port;
    let url = if port == 80 {
        "http://localhost/".to_owned()
    } else {
        format!("http://localhost:{port}/")
    };
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| Error::Other(e.to_string()))?;
    Ok(())
}

/// Shared with the tray menu.
pub fn open_www_for(app: &AppHandle) -> CommandResult<()> {
    let rt = app.state::<AppState>().runtime()?;
    let www = rt.stack.www_root(&rt.config());
    open_path(app, &www.to_string_lossy())
}

fn open_path(app: &AppHandle, path: &str) -> CommandResult<()> {
    app.opener()
        .open_path(path, None::<&str>)
        .map_err(|e| Error::Other(e.to_string()))?;
    Ok(())
}
