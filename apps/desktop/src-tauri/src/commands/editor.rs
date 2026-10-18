//! Launch the user's custom editor for a project or config file.
//!
//! The editor starts detached, with Lumina's shim folder first on its PATH so
//! its integrated terminal finds `php`, `composer` and `node` right away.

use std::path::PathBuf;

use lumina_config::schema::DatabaseEngine;
use lumina_core::Error;
use lumina_platform::launcher::{self, EditorPreset};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::CommandResult;
use crate::state::{AppState, Runtime};

/// What to open.
#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum OpenTarget {
    /// A project folder, by its folder name in the projects folder.
    Site { name: String },
    /// One of the files the user may edit.
    Config { file: ConfigFile },
    /// The projects folder itself.
    Www,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ConfigFile {
    /// `config.toml`.
    Lumina,
    /// php.ini of the active PHP.
    PhpIni,
    /// `custom.cnf` of the configured database engine (my.cnf includes it).
    Database,
    /// Redis `custom.conf` (redis.conf includes it).
    Redis,
    /// `postgresql.conf` in the data folder of the selected PostgreSQL.
    Postgresql,
    /// The web server's `sites/` folder (vhost files; `<domain>.conf` overrides).
    WebSites,
}

#[tauri::command]
#[specta::specta]
pub fn editor_presets() -> Vec<EditorPreset> {
    launcher::editor_presets()
}

/// The file or folder `target` means, creating an empty user file if needed.
async fn resolve_target(rt: &Runtime, target: &OpenTarget) -> lumina_core::Result<PathBuf> {
    let cfg = rt.config();
    let etc = rt.layout.etc();
    let ensure = |path: PathBuf| -> lumina_core::Result<PathBuf> {
        if !path.exists() {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|source| Error::Io {
                    path: parent.to_path_buf(),
                    source,
                })?;
            }
            std::fs::write(&path, b"").map_err(|source| Error::Io {
                path: path.clone(),
                source,
            })?;
        }
        Ok(path)
    };
    Ok(match target {
        OpenTarget::Www => rt.stack.www_root(&cfg),
        OpenTarget::Site { name } => {
            let site = rt
                .stack
                .sites(&cfg)
                .sites
                .into_iter()
                .find(|s| &s.name == name)
                .ok_or_else(|| Error::Other(format!("no project named {name}")))?;
            site.path
        }
        OpenTarget::Config { file } => match file {
            ConfigFile::Lumina => rt.layout.config_file(),
            ConfigFile::PhpIni => {
                rt.stack
                    .php()
                    .cli(&cfg.php)
                    .await
                    .ok_or_else(|| Error::not_installed("php", "PHP"))?
                    .1
            }
            ConfigFile::Database => {
                let engine = match cfg.database.engine {
                    DatabaseEngine::Mysql => "mysql",
                    DatabaseEngine::Mariadb => "mariadb",
                };
                ensure(etc.join(engine).join("custom.cnf"))?
            }
            ConfigFile::Redis => ensure(etc.join("redis").join("custom.conf"))?,
            ConfigFile::Postgresql => {
                let pg = rt.stack.postgresql();
                let installs = pg.versions().await;
                let install = lumina_services::install::select(&installs, &cfg.postgresql.version)
                    .ok_or_else(|| Error::not_installed("postgresql", "PostgreSQL"))?;
                let file = pg.data_dir(install).join("postgresql.conf");
                if !file.is_file() {
                    return Err(Error::Other(
                        "start PostgreSQL once; its first start creates postgresql.conf".into(),
                    ));
                }
                file
            }
            ConfigFile::WebSites => match cfg.web.server {
                lumina_config::schema::WebServer::Nginx => rt.stack.nginx().sites_dir(),
                lumina_config::schema::WebServer::Apache => rt.stack.apache().sites_dir(),
            },
        },
    })
}

/// Opens `target` in the editor from the settings (`[editor]`).
#[tauri::command]
#[specta::specta]
pub async fn editor_open(
    state: State<'_, AppState>,
    target: OpenTarget,
    line: Option<u32>,
) -> CommandResult<()> {
    open(&*state.runtime()?, &target, line).await
}

/// [`editor_open`], shared with the tray.
pub async fn open(rt: &Runtime, target: &OpenTarget, line: Option<u32>) -> CommandResult<()> {
    let path = resolve_target(rt, target).await?;
    let editor = rt.config().editor;
    let program = launcher::resolve(&editor.command).ok_or_else(|| {
        Error::Other(format!(
            "the editor command \"{}\" was not found; choose an editor in Settings",
            editor.command
        ))
    })?;
    let args = launcher::editor_args(&editor.args, &editor.args_line, &path, line);
    let cwd = if path.is_dir() {
        Some(path.as_path())
    } else {
        path.parent()
    };
    launcher::spawn_detached(&program, &args, cwd, Some(&rt.stack.shim_dir()))
        .map_err(|e| Error::Other(format!("could not start {}: {e}", editor.name)))?;
    log::info!("opened {} in {}", path.display(), editor.name);
    Ok(())
}
