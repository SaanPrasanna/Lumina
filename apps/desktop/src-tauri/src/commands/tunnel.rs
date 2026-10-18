//! Start/stop Ngrok tunnels.
//!
//! "Share" on the Sites page opens a public HTTPS address for one site: the
//! agent forwards it to the web server's plain-HTTP port on 127.0.0.1 with
//! the `Host` rewritten to `<site>.test`, and the web server templates give
//! PHP the public host (see `lumina_tunnel`). The authtoken stays in the
//! system keychain; ngrok itself is installed from Quick Add.

use std::path::PathBuf;

use lumina_tunnel::cloudflare;
use lumina_tunnel::ngrok::{self, Settings};
use lumina_tunnel::{Tunnel, TunnelProvider, TunnelSpec};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_opener::OpenerExt;

use crate::error::{CommandError, CommandResult};
use crate::events;
use crate::state::{AppState, Runtime};
use lumina_services::orchestrator::Orchestrator;

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TunnelView {
    #[serde(flatten)]
    pub tunnel: Tunnel,
    /// A QR code of the public URL, as an SVG document.
    pub qr_svg: String,
}

#[derive(Debug, Serialize, specta::Type)]
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
    /// `ngrok` or `cloudflare` (`[tunnel] provider`).
    pub provider: String,
    /// The cloudflared version that runs, if one is installed.
    pub cloudflared_installed: Option<String>,
    /// The newest cloudflared Quick Add can install on this system.
    pub cloudflared_available: Option<String>,
}

/// Whether sharing uses Cloudflare quick tunnels instead of ngrok.
fn uses_cloudflare(rt: &Runtime) -> bool {
    rt.config().tunnel.provider == "cloudflare"
}

/// The cloudflared program and its version: `[tools] cloudflared`, else the newest.
fn cloudflared(rt: &Runtime) -> Option<(String, PathBuf)> {
    let wanted = rt.config().tools.cloudflared;
    let installed = rt.packages.installed("cloudflared");
    let pick = installed
        .iter()
        .find(|i| !wanted.is_empty() && i.version == wanted)
        .or_else(|| installed.first())?;
    let exe = pick.dir.join(cloudflare::program_name());
    exe.is_file().then(|| (pick.version.clone(), exe))
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
    available_of(rt, "ngrok")
}

fn available_of(rt: &Runtime, id: &str) -> Option<String> {
    let (os, arch) = lumina_packages::catalog::platform();
    rt.packages
        .catalog()
        .package(id)?
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
#[specta::specta]
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
        tunnels: rt
            .tunnels
            .tunnels()
            .await
            .into_iter()
            .chain(rt.cloudflare.tunnels().await)
            .map(view)
            .collect(),
        stopped: rt.tunnels.stopped_reason(),
        log_file: rt.tunnels.log_file().to_path_buf(),
        provider: rt.config().tunnel.provider,
        cloudflared_installed: cloudflared(&rt).map(|(version, _)| version),
        cloudflared_available: available_of(&rt, "cloudflared"),
    })
}

/// Chooses how sites are shared: `ngrok` or `cloudflare`.
#[tauri::command]
#[specta::specta]
pub fn tunnel_set_provider(
    app: AppHandle,
    state: State<'_, AppState>,
    provider: String,
) -> CommandResult<()> {
    if provider != "ngrok" && provider != "cloudflare" {
        return Err(
            lumina_core::Error::Other(format!("unknown tunnel provider `{provider}`")).into(),
        );
    }
    let rt = state.runtime()?;
    let mut cfg = rt.config();
    cfg.tunnel.provider = provider;
    lumina_config::save(&rt.layout.config_file(), &cfg)?;
    *rt.config.write().unwrap_or_else(|e| e.into_inner()) = cfg;
    let _ = app.emit(events::CONFIG_CHANGED, "tunnel");
    let _ = app.emit(events::TUNNELS_CHANGED, ());
    Ok(())
}

/// Saves the authtoken in the system keychain (replacing a saved one).
#[tauri::command]
#[specta::specta]
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
#[specta::specta]
pub async fn tunnel_clear_token(app: AppHandle, state: State<'_, AppState>) -> CommandResult<()> {
    let rt = state.runtime()?;
    rt.tunnel_token.delete().await.map_err(tunnel_error)?;
    log::info!("ngrok authtoken removed from the system keychain");
    let _ = app.emit(events::TUNNELS_CHANGED, ());
    Ok(())
}

/// Saves the reserved domain offered when sharing (`""` = none).
#[tauri::command]
#[specta::specta]
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
#[specta::specta]
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
    let use_cloudflare = uses_cloudflare(&rt);
    let program = if use_cloudflare {
        cloudflared(&rt).map(|(_, p)| p)
    } else {
        program(&rt).map(|(_, p)| p)
    };
    let Some(program) = program else {
        return Err(CommandError {
            code: "tunnel_not_installed",
            message: if use_cloudflare {
                "cloudflared is not installed".into()
            } else {
                "ngrok is not installed".into()
            },
            details: None,
        });
    };
    let authtoken = if use_cloudflare {
        String::new()
    } else {
        rt.tunnel_token
            .load()
            .await
            .map_err(tunnel_error)?
            .ok_or_else(|| tunnel_error(lumina_tunnel::Error::NoToken))?
    };
    // Quick tunnels always get a random address.
    let domain = match domain.filter(|_| !use_cloudflare) {
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
    let result = if use_cloudflare {
        rt.cloudflare
            .open(&cloudflare::Settings { program }, &spec)
            .await
    } else {
        rt.tunnels
            .open(&Settings { program, authtoken }, &spec)
            .await
    };
    let _ = app.emit(events::TUNNELS_CHANGED, ());
    Ok(view(result.map_err(tunnel_error)?))
}

#[tauri::command]
#[specta::specta]
pub async fn tunnel_stop(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> CommandResult<()> {
    let rt = state.runtime()?;
    let on_cloudflare = rt.cloudflare.tunnels().await.iter().any(|t| t.name == name);
    let result = if on_cloudflare {
        rt.cloudflare.close(&name).await
    } else {
        rt.tunnels.close(&name).await
    };
    let _ = app.emit(events::TUNNELS_CHANGED, ());
    result.map_err(tunnel_error)
}

#[tauri::command]
#[specta::specta]
pub async fn tunnel_stop_all(app: AppHandle, state: State<'_, AppState>) -> CommandResult<()> {
    let rt = state.runtime()?;
    let ngrok = rt.tunnels.shutdown().await;
    let cloudflare = rt.cloudflare.shutdown().await;
    let _ = app.emit(events::TUNNELS_CHANGED, ());
    ngrok.and(cloudflare).map_err(tunnel_error)
}

/// Opens the agent's log in the system's default viewer.
#[tauri::command]
#[specta::specta]
pub fn tunnel_open_log(app: AppHandle, state: State<'_, AppState>) -> CommandResult<()> {
    let rt = state.runtime()?;
    // Cloudflare: the newest tunnel log (one per shared site).
    let newest_cloudflare = || {
        std::fs::read_dir(rt.layout.logs())
            .ok()?
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with("cloudflared-"))
            .max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok())
            .map(|e| e.path())
    };
    let log = if uses_cloudflare(&rt) {
        newest_cloudflare().unwrap_or_else(|| rt.layout.logs().join("cloudflared.log"))
    } else {
        rt.tunnels.log_file().to_path_buf()
    };
    let log = log.as_path();
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
