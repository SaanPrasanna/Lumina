//! HTTPS for every site: switch it on and off, trust or remove the local CA.
//!
//! Certificates are created by the orchestrator whenever the web server's
//! config is rendered; this module only handles the user's decisions, each of
//! which ends with one site sync (a single reload).

use std::path::PathBuf;
use std::sync::Arc;

use lumina_config::schema::General;
use lumina_core::Error;
use lumina_platform::trust_store::{self, CaCert, TrustError};
use lumina_ssl::CaInfo;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::commands::sites;
use crate::error::CommandResult;
use crate::events;
use crate::state::{AppState, Runtime};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaStatus {
    #[serde(flatten)]
    pub info: CaInfo,
    /// The OS trusts it, so browsers show a padlock.
    pub trusted: bool,
    /// It may sign for the configured domain suffix.
    pub covers_tld: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteCert {
    pub domain: String,
    /// Unix seconds.
    pub not_after: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SslOverview {
    pub enabled: bool,
    pub force_https: bool,
    pub https_port: u16,
    pub tld: String,
    pub ca: Option<CaStatus>,
    /// Certificates on disk for the current sites (issued when the web
    /// server renders its config).
    pub sites: Vec<SiteCert>,
    /// Why HTTPS could not be set up, if it failed.
    pub error: Option<String>,
    pub ssl_dir: PathBuf,
}

/// What a trust change did.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TrustOutcome {
    Done,
    /// The user answered "No" to the OS prompt; nothing changed.
    Declined,
}

fn ca_status(rt: &Runtime, tld: &str) -> lumina_core::Result<Option<CaStatus>> {
    let Some(ca) = rt.stack.ssl().ca()? else {
        return Ok(None);
    };
    let trusted = trust_store::is_trusted(&cert(&ca.info));
    Ok(Some(CaStatus {
        covers_tld: ca.info.covers(tld),
        info: ca.info,
        trusted,
    }))
}

fn cert(info: &CaInfo) -> CaCert<'_> {
    CaCert {
        der: &info.der,
        pem_path: &info.cert_path,
        sha1: info.sha1,
    }
}

#[tauri::command]
pub async fn ssl_status(state: State<'_, AppState>) -> CommandResult<SslOverview> {
    let rt = state.runtime()?;
    let cfg = rt.config();
    let ca = {
        let rt = rt.clone();
        let tld = cfg.general.tld.clone();
        tauri::async_runtime::spawn_blocking(move || ca_status(&rt, &tld))
            .await
            .map_err(|e| Error::Other(e.to_string()))??
    };
    let sites = rt
        .stack
        .sites(&cfg)
        .sites
        .into_iter()
        .filter_map(|s| {
            rt.stack.ssl().site(&s.domain).map(|leaf| SiteCert {
                domain: s.domain,
                not_after: leaf.not_after,
            })
        })
        .collect();
    Ok(SslOverview {
        enabled: cfg.general.auto_ssl,
        force_https: cfg.general.force_https,
        https_port: cfg.web.https_port,
        tld: cfg.general.tld.clone(),
        ca,
        sites,
        error: rt.stack.tls_error(),
        ssl_dir: rt.stack.ssl().dir().to_path_buf(),
    })
}

/// Turns HTTPS on or off. Turning it on creates the CA if needed and asks
/// the OS to trust it (one prompt) when it is not trusted yet.
#[tauri::command]
pub async fn ssl_set_enabled(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> CommandResult<Option<TrustOutcome>> {
    let rt = state.runtime()?;
    save_general(&app, &rt, |g| g.auto_ssl = enabled)?;
    let outcome = if enabled {
        let cfg = rt.config();
        let ca = rt
            .stack
            .ssl()
            .ensure_ca(
                &lumina_services::orchestrator::host_name(),
                &cfg.general.tld,
            )
            .map_err(lumina_core::Error::from)?
            .0;
        Some(trust(&rt, ca.info).await?)
    } else {
        None
    };
    sites::resync_later(&app);
    Ok(outcome)
}

/// Redirect plain HTTP to HTTPS on every site.
#[tauri::command]
pub fn ssl_set_force_https(
    app: AppHandle,
    state: State<'_, AppState>,
    force: bool,
) -> CommandResult<()> {
    let rt = state.runtime()?;
    save_general(&app, &rt, |g| g.force_https = force)?;
    sites::resync_later(&app);
    Ok(())
}

/// Asks the OS to trust the current CA (one prompt).
#[tauri::command]
pub async fn ssl_trust(state: State<'_, AppState>) -> CommandResult<TrustOutcome> {
    let rt = state.runtime()?;
    let ca = rt
        .stack
        .ssl()
        .ca()
        .map_err(lumina_core::Error::from)?
        .ok_or_else(|| Error::Other("there is no local CA yet; turn HTTPS on first".into()))?;
    Ok(trust(&rt, ca.info).await?)
}

/// Replaces the CA: the old one is removed from the OS store, a new one is
/// created and trusted, and every site gets a new certificate.
#[tauri::command]
pub async fn ssl_regenerate(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<TrustOutcome> {
    let rt = state.runtime()?;
    if let Some(old) = rt.stack.ssl().ca().map_err(lumina_core::Error::from)?
        && let TrustOutcome::Declined = untrust(&rt, old.info).await?
    {
        return Ok(TrustOutcome::Declined);
    }
    let cfg = rt.config();
    let ca = rt
        .stack
        .ssl()
        .create_ca(
            &lumina_services::orchestrator::host_name(),
            &cfg.general.tld,
        )
        .map_err(lumina_core::Error::from)?;
    let outcome = trust(&rt, ca.info).await?;
    sites::resync_later(&app);
    Ok(outcome)
}

/// Removes the CA from the OS store, deletes it and every certificate, and
/// turns HTTPS off.
#[tauri::command]
pub async fn ssl_remove_ca(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<TrustOutcome> {
    let rt = state.runtime()?;
    if let Some(ca) = rt.stack.ssl().ca().map_err(lumina_core::Error::from)?
        && let TrustOutcome::Declined = untrust(&rt, ca.info).await?
    {
        return Ok(TrustOutcome::Declined);
    }
    save_general(&app, &rt, |g| g.auto_ssl = false)?;
    // Plain HTTP first, so the server no longer points at files about to go.
    sites::resync(&app, false).await?;
    rt.stack
        .ssl()
        .remove_all()
        .map_err(lumina_core::Error::from)?;
    let _ = app.emit(events::SITES_CHANGED, ());
    Ok(TrustOutcome::Done)
}

async fn trust(rt: &Arc<Runtime>, info: CaInfo) -> lumina_core::Result<TrustOutcome> {
    let helper = rt.helper.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let ca = cert(&info);
        if trust_store::is_trusted(&ca) {
            return Ok(TrustOutcome::Done);
        }
        match trust_store::install(&ca, &helper) {
            Ok(()) => {
                log::info!("the OS now trusts {}", info.subject);
                Ok(TrustOutcome::Done)
            }
            Err(TrustError::Declined) => Ok(TrustOutcome::Declined),
            Err(e) => Err(Error::Other(format!("could not trust the local CA: {e}"))),
        }
    })
    .await
    .map_err(|e| Error::Other(e.to_string()))?
}

async fn untrust(rt: &Arc<Runtime>, info: CaInfo) -> lumina_core::Result<TrustOutcome> {
    let helper = rt.helper.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let ca = cert(&info);
        if !trust_store::is_trusted(&ca) {
            return Ok(TrustOutcome::Done);
        }
        match trust_store::remove(&ca, &helper) {
            Ok(()) => {
                log::info!("removed {} from the OS trust store", info.subject);
                Ok(TrustOutcome::Done)
            }
            Err(TrustError::Declined) => Ok(TrustOutcome::Declined),
            Err(e) => Err(Error::Other(format!("could not remove the local CA: {e}"))),
        }
    })
    .await
    .map_err(|e| Error::Other(e.to_string()))?
}

/// Changes `[general]`, validates, saves config.toml and tells the UI.
fn save_general(
    app: &AppHandle,
    rt: &Runtime,
    change: impl FnOnce(&mut General),
) -> lumina_core::Result<()> {
    let mut config = rt.config();
    change(&mut config.general);
    let warnings = lumina_config::validate::validate(&mut config);
    lumina_config::save(&rt.layout.config_file(), &config)?;
    *rt.config.write().unwrap_or_else(|e| e.into_inner()) = config;
    *rt.warnings.write().unwrap_or_else(|e| e.into_inner()) = warnings;
    let _ = app.emit(events::CONFIG_CHANGED, "general");
    Ok(())
}
