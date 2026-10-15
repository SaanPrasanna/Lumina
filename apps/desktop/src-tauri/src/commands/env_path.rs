//! Tools > PATH > Add/Remove app binaries to PATH.
//!
//! One folder, `<data-root>/bin/.shims`, goes on the user's PATH. Its shims
//! follow the active versions and are rewritten whenever those change (here
//! via [`refresh_shims_later`]), so PATH itself is edited only by Add/Remove.

use std::path::PathBuf;

use lumina_core::Error;
use lumina_platform::path_env::{self, PathStatus};
use lumina_services::tools::Shim;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::error::CommandResult;
use crate::state::{AppState, Runtime};

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PathOverview {
    pub shim_dir: PathBuf,
    #[serde(flatten)]
    pub status: PathStatus,
    /// What each command runs now.
    pub shims: Vec<Shim>,
    /// `false` in development builds without `lumina-shim` (`.cmd` launchers
    /// are written instead).
    pub native_shims: bool,
}

async fn overview(rt: &Runtime) -> lumina_core::Result<PathOverview> {
    let cfg = rt.config();
    let shims = rt.stack.refresh_shims(&cfg, Some(&rt.shim_exe)).await?;
    let dir = rt.stack.shim_dir();
    let status = {
        let dir = dir.clone();
        // Unix asks a login shell for its PATH; keep that off the async runtime.
        tauri::async_runtime::spawn_blocking(move || path_env::status(&dir))
            .await
            .map_err(|e| Error::Other(e.to_string()))?
    };
    Ok(PathOverview {
        shim_dir: dir,
        status,
        shims,
        native_shims: rt.shim_exe.is_file(),
    })
}

#[tauri::command]
#[specta::specta]
pub async fn path_status(state: State<'_, AppState>) -> CommandResult<PathOverview> {
    let rt = state.runtime()?;
    Ok(overview(&rt).await?)
}

/// Puts the shim folder first on the user's PATH (new terminals see it).
#[tauri::command]
#[specta::specta]
pub async fn path_add(state: State<'_, AppState>) -> CommandResult<PathOverview> {
    let rt = state.runtime()?;
    let cfg = rt.config();
    rt.stack.refresh_shims(&cfg, Some(&rt.shim_exe)).await?;
    let changed = path_env::add(&rt.stack.shim_dir())
        .map_err(|e| Error::Other(format!("could not change PATH: {e}")))?;
    log::info!(
        "added {} to PATH ({} changed)",
        rt.stack.shim_dir().display(),
        changed.len()
    );
    Ok(overview(&rt).await?)
}

/// Takes the shim folder off PATH again, restoring what was there.
#[tauri::command]
#[specta::specta]
pub async fn path_remove(state: State<'_, AppState>) -> CommandResult<PathOverview> {
    let rt = state.runtime()?;
    // The machine-wide entry needs administrator rights; only ask if it exists.
    if path_env::status(&rt.stack.shim_dir()).added_system {
        helper(&rt, "path-system-remove").await?;
    }
    let changed = path_env::remove(&rt.stack.shim_dir())
        .map_err(|e| Error::Other(format!("could not change PATH: {e}")))?;
    log::info!(
        "removed {} from PATH ({} changed)",
        rt.stack.shim_dir().display(),
        changed.len()
    );
    Ok(overview(&rt).await?)
}

async fn helper(rt: &Runtime, command: &str) -> lumina_core::Result<bool> {
    let helper = rt.helper.clone();
    let dir = rt.stack.shim_dir();
    let command = command.to_owned();
    let result = tauri::async_runtime::spawn_blocking(move || {
        lumina_platform::elevation::run_elevated(&helper, &[command.as_ref(), dir.as_os_str()])
    })
    .await
    .map_err(|e| Error::Other(e.to_string()))?;
    match result {
        Ok(exit) if exit.code == 0 => Ok(true),
        Ok(exit) => Err(Error::Other(format!(
            "lumina-helper could not change the machine PATH (exit code {}) {}",
            exit.code, exit.stderr
        ))),
        Err(lumina_platform::elevation::ElevationError::Declined) => Ok(false),
        Err(e) => Err(Error::Other(e.to_string())),
    }
}

/// Windows: also puts the shim folder first on the machine-wide PATH, which
/// Windows searches before the user PATH, so a PHP or Node.js installed for
/// all users no longer comes first. One administrator prompt.
#[tauri::command]
#[specta::specta]
pub async fn path_add_system(state: State<'_, AppState>) -> CommandResult<PathOverview> {
    let rt = state.runtime()?;
    let cfg = rt.config();
    rt.stack.refresh_shims(&cfg, Some(&rt.shim_exe)).await?;
    if helper(&rt, "path-system-add").await? {
        log::info!(
            "added {} to the machine PATH",
            rt.stack.shim_dir().display()
        );
    }
    Ok(overview(&rt).await?)
}

/// Opens a terminal in `path` (default: the projects folder) with Lumina's
/// commands first on PATH.
#[tauri::command]
#[specta::specta]
pub async fn terminal_open(state: State<'_, AppState>, path: Option<PathBuf>) -> CommandResult<()> {
    open_terminal_for(&*state.runtime()?, path).await
}

/// [`terminal_open`], shared with the tray.
pub async fn open_terminal_for(
    rt: &crate::state::Runtime,
    path: Option<PathBuf>,
) -> CommandResult<()> {
    let cfg = rt.config();
    rt.stack.refresh_shims(&cfg, Some(&rt.shim_exe)).await?;
    let dir = path
        .filter(|p| p.is_dir())
        .unwrap_or_else(|| rt.stack.www_root(&cfg));
    lumina_platform::launcher::open_terminal(&dir, &rt.stack.shim_dir())
        .map_err(|e| Error::Other(format!("could not open a terminal: {e}")))?;
    Ok(())
}

/// Rewrites the shims (and Composer launchers) in the background, e.g. after
/// a PHP switch, an install or a profile change.
pub fn refresh_shims_later(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let Ok(rt) = app.state::<AppState>().runtime() else {
            return;
        };
        if let Err(err) = rt
            .stack
            .refresh_shims(&rt.config(), Some(&rt.shim_exe))
            .await
        {
            log::warn!("could not update the PATH shims: {err}");
        }
    });
}
