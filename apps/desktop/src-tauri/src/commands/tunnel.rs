//! Start/stop Ngrok tunnels.
//!
//! "Share" on the Sites page opens a public HTTPS address for one site: the
//! agent forwards it to the web server's plain-HTTP port on 127.0.0.1 with
//! the `Host` rewritten to `<site>.test`, and the web server templates give
//! PHP the public host (see `lumina_tunnel`). The authtoken stays in the
//! system keychain; ngrok itself is installed from Quick Add.

use std::path::PathBuf;

use lumina_tunnel::ngrok::{self, Settings};
use lumina_tunnel::{Tunnel, TunnelProvider, TunnelSpec};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_opener::OpenerExt;

use crate::error::{CommandError, CommandResult};
use crate::events;
use crate::state::{AppState, Runtime};
use lumina_services::orchestrator::Orchestrator;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelView {
    #[serde(flatten)]
    pub tunnel: Tunnel,
    /// A QR code of the public URL, as an SVG document.
    pub qr_svg: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelOverview {
    /// The ngrok version that runs, if one is installed.
    pub installed: Option<String>,
    /// The newest version Quick Add can install on this system.
    pub available: Option<String>,
    pub has_token: bool,
    /// The keychain could not be read.
    pub token_error: Option<String>,
    /// The saved reserved domain (`[tunnel] domain`), or empty.
    pub domain: String,
    pub tunnels: Vec<TunnelView>,
    /// Why the agent stopped by itself, if it did.
    pub stopped: Option<String>,
    pub log_file: PathBuf,
}

fn tunnel_error(err: lumina_tunnel::Error) -> CommandError {
    match err {
        lumina_tunnel::Error::Core(e) => e.into(),
        lumina_tunnel::Error::Io { path, source } => lumina_core::Error::Io { path, source }.into(),
        err => {
            log::warn!("tunnel command failed [{}]: {err}", err.code());
            CommandError {
                code: err.code(),
                message: err.to_string(),
                details: None,
            }
        }
    }
}

/// The ngrok program and its version: `[tools] ngrok`, else the newest installed.
fn program(rt: &Runtime) -> Option<(String, PathBuf)> {
    let wanted = rt.config().tools.ngrok;
    let installed = rt.packages.installed("ngrok");
    let pick = installed
        .iter()
        .find(|i| !wanted.is_empty() && i.version == wanted)
        .or_else(|| installed.first())?;
    let exe = pick.dir.join(ngrok::program_name());
    exe.is_file().then(|| (pick.version.clone(), exe))
}

fn available(rt: &Runtime) -> Option<String> {
    let (os, arch) = lumina_packages::catalog::platform();
    rt.packages
        .catalog()
        .package("ngrok")?
        .versions
        .iter()
        .find(|r| r.fits(os, arch))
        .map(|r| r.version.clone())
}

fn view(tunnel: Tunnel) -> TunnelView {
    let qr_svg = lumina_tunnel::qr::svg(&tunnel.public_url).unwrap_or_default();
    TunnelView { tunnel, qr_svg }
}

#[tauri::command]
pub async fn tunnel_overview(state: State<'_, AppState>) -> CommandResult<TunnelOverview> {
    let rt = state.runtime()?;
    let (has_token, token_error) = match rt.tunnel_token.load().await {
        Ok(token) => (token.is_some(), None),
        Err(e) => (false, Some(e.to_string())),
    };
    Ok(TunnelOverview {
        installed: program(&rt).map(|(version, _)| version),
        available: available(&rt),
        has_token,
        token_error,
        domain: rt.config().tunnel.domain,
        tunnels: rt.tunnels.tunnels().await.into_iter().map(view).collect(),
        stopped: rt.tunnels.stopped_reason(),
        log_file: rt.tunnels.log_file().to_path_buf(),
    })
}

/// Saves the authtoken in the system keychain (replacing a saved one).
#[tauri::command]
pub async fn tunnel_set_token(
    app: AppHandle,
    state: State<'_, AppState>,
    token: String,
) -> CommandResult<()> {
    let rt = state.runtime()?;
    rt.tunnel_token.save(&token).await.map_err(tunnel_error)?;
    log::info!("ngrok authtoken saved in the system keychain");
    let _ = app.emit(events::TUNNELS_CHANGED, ());
    Ok(())
}

/// Removes the authtoken from the keychain; open tunnels keep running.
#[tauri::command]
pub async fn tunnel_clear_token(app: AppHandle, state: State<'_, AppState>) -> CommandResult<()> {
    let rt = state.runtime()?;
    rt.tunnel_token.delete().await.map_err(tunnel_error)?;
    log::info!("ngrok authtoken removed from the system keychain");
    let _ = app.emit(events::TUNNELS_CHANGED, ());
    Ok(())
}

/// Saves the reserved domain offered when sharing (`""` = none).
#[tauri::command]
pub fn tunnel_set_domain(
    app: AppHandle,
    state: State<'_, AppState>,
    domain: String,
) -> CommandResult<String> {
    let rt = state.runtime()?;
    let domain = ngrok::normalize_domain(&domain)
        .map_err(tunnel_error)?
        .unwrap_or_default();
    let mut cfg = rt.config();
    cfg.tunnel.domain = domain.clone();
    lumina_config::save(&rt.layout.config_file(), &cfg)?;
    *rt.config.write().unwrap_or_else(|e| e.into_inner()) = cfg;
    let _ = app.emit(events::CONFIG_CHANGED, "tunnel");
    let _ = app.emit(events::TUNNELS_CHANGED, ());
    Ok(domain)
}

/// Shares site `name` (its folder name), starting the web server if needed.
/// `domain`: a reserved ngrok domain to use, or none for a random address.
#[tauri::command]
pub async fn tunnel_start(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    domain: Option<String>,
) -> CommandResult<TunnelView> {
    let rt = state.runtime()?;
    let cfg = rt.config();
    let site = rt
        .stack
        .sites(&cfg)
        .sites
        .into_iter()
        .find(|s| s.name == name)
        .ok_or_else(|| lumina_core::Error::Other(format!("no site named `{name}`")))?;
    let Some((_, program)) = program(&rt) else {
        return Err(CommandError {
            code: "tunnel_not_installed",
            message: "ngrok is not installed".into(),
            details: None,
        });
    };
    let authtoken = rt
        .tunnel_token
        .load()
        .await
        .map_err(tunnel_error)?
        .ok_or_else(|| tunnel_error(lumina_tunnel::Error::NoToken))?;
    let domain = match domain {
        Some(d) => ngrok::normalize_domain(&d).map_err(tunnel_error)?,
        None => None,
    };

    // The tunnel forwards to the web server: it has to run.
    let kind = match rt.stack.running_web().await {
        Some(kind) => kind,
        None => {
            let kind = Orchestrator::configured_web(&cfg);
            rt.stack.start(kind, &cfg).await?;
            kind
        }
    };
    let port = rt
        .stack
        .web_status(kind)
        .await
        .runtime
        .map(|r| r.http_port)
        .unwrap_or(cfg.web.http_port);

    let spec = TunnelSpec {
        name: site.name.clone(),
        host: site.domain.clone(),
        port,
        domain,
    };
    let result = rt
        .tunnels
        .open(&Settings { program, authtoken }, &spec)
        .await;
    let _ = app.emit(events::TUNNELS_CHANGED, ());
    Ok(view(result.map_err(tunnel_error)?))
}

#[tauri::command]
pub async fn tunnel_stop(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> CommandResult<()> {
    let rt = state.runtime()?;
    let result = rt.tunnels.close(&name).await;
    let _ = app.emit(events::TUNNELS_CHANGED, ());
    result.map_err(tunnel_error)
}

#[tauri::command]
pub async fn tunnel_stop_all(app: AppHandle, state: State<'_, AppState>) -> CommandResult<()> {
    let rt = state.runtime()?;
    let result = rt.tunnels.shutdown().await;
    let _ = app.emit(events::TUNNELS_CHANGED, ());
    result.map_err(tunnel_error)
}

/// Opens the agent's log in the system's default viewer.
#[tauri::command]
pub fn tunnel_open_log(app: AppHandle, state: State<'_, AppState>) -> CommandResult<()> {
    let rt = state.runtime()?;
    let log = rt.tunnels.log_file();
    if !log.exists() {
        return Err(
            lumina_core::Error::Other(format!("{} does not exist yet", log.display())).into(),
        );
    }
    app.opener()
        .open_path(log.to_string_lossy(), None::<&str>)
        .map_err(|e| lumina_core::Error::Other(e.to_string()))?;
    Ok(())
}
