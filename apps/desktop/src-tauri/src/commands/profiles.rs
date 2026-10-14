//! List/save/switch stack profiles.
//!
//! A switch applies the profile to a copy of the config, lets the
//! orchestrator change only what differs (rolling back on failure), and saves
//! the config only when the new stack is up.

use lumina_config::AppConfig;
use lumina_config::schema::{DatabaseEngine, Profile, WebServer};
use lumina_core::Error;
use lumina_services::install::{self, Install};
use lumina_services::profiles::Plan;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::error::CommandResult;
use crate::events;
use crate::state::{AppState, Runtime};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileView {
    #[serde(flatten)]
    pub profile: Profile,
    /// The configuration runs exactly this stack.
    pub current: bool,
    /// What is not installed, e.g. "PHP 8.2".
    pub missing: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfilesOverview {
    pub profiles: Vec<ProfileView>,
    /// The stack the configuration describes now.
    pub stack: Profile,
    /// Name of the profile last switched to.
    pub active: String,
}

fn wanted(label: &str, version: &str) -> String {
    if version.trim().is_empty() {
        label.to_owned()
    } else {
        format!("{label} {version}")
    }
}

/// What `profile` needs that is not installed.
pub async fn missing(rt: &Runtime, profile: &Profile) -> Vec<String> {
    let stack = &rt.stack;
    let mut out = Vec::new();
    let check = |installs: &[Install], label: &str, version: &str, out: &mut Vec<String>| {
        if install::select(installs, version).is_none() {
            out.push(wanted(label, version));
        }
    };
    check(&stack.php().versions().await, "PHP", &profile.php, &mut out);
    let (web, label) = match profile.web {
        WebServer::Nginx => (stack.nginx().versions().await, "nginx"),
        WebServer::Apache => (stack.apache().versions().await, "Apache"),
    };
    check(&web, label, &profile.web_version, &mut out);
    let engine = profile.database.engine;
    let label = match engine {
        DatabaseEngine::Mysql => "MySQL",
        DatabaseEngine::Mariadb => "MariaDB",
    };
    check(
        &stack.database(engine).versions().await,
        label,
        &profile.database.version,
        &mut out,
    );
    if profile.redis {
        check(&stack.redis().versions().await, "Redis", "", &mut out);
    }
    out
}

#[tauri::command]
pub async fn profiles_list(state: State<'_, AppState>) -> CommandResult<ProfilesOverview> {
    let rt = state.runtime()?;
    let cfg = rt.config();
    let mut profiles = Vec::new();
    for profile in &cfg.profiles.list {
        profiles.push(ProfileView {
            current: profile.matches(&cfg),
            missing: missing(&rt, profile).await,
            profile: profile.clone(),
        });
    }
    Ok(ProfilesOverview {
        profiles,
        stack: Profile::capture(&cfg, ""),
        active: cfg.profiles.active.clone(),
    })
}

/// Validates and saves a config change to the profile list.
fn save(app: &AppHandle, rt: &Runtime, cfg: AppConfig) -> lumina_core::Result<()> {
    let mut cfg = cfg;
    let warnings = lumina_config::validate::validate(&mut cfg);
    lumina_config::save(&rt.layout.config_file(), &cfg)?;
    *rt.config.write().unwrap_or_else(|e| e.into_inner()) = cfg;
    *rt.warnings.write().unwrap_or_else(|e| e.into_inner()) = warnings;
    let _ = app.emit(events::CONFIG_CHANGED, "profiles");
    crate::tray::refresh(app.clone());
    Ok(())
}

fn check_name(cfg: &AppConfig, name: &str, replacing: Option<&str>) -> lumina_core::Result<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::Other("a profile needs a name".into()));
    }
    let taken = cfg.profiles.list.iter().any(|p| {
        p.name.eq_ignore_ascii_case(name)
            && replacing.is_none_or(|r| !p.name.eq_ignore_ascii_case(r))
    });
    if taken {
        return Err(Error::Other(format!(
            "there is already a profile named \"{name}\""
        )));
    }
    Ok(name.to_owned())
}

/// Adds `profile`, or replaces the one named `original` (renaming is allowed).
#[tauri::command]
pub fn profile_save(
    app: AppHandle,
    state: State<'_, AppState>,
    profile: Profile,
    original: Option<String>,
) -> CommandResult<()> {
    let rt = state.runtime()?;
    let mut cfg = rt.config();
    let mut profile = profile;
    profile.name = check_name(&cfg, &profile.name, original.as_deref())?;
    match original
        .as_deref()
        .and_then(|o| cfg.profiles.list.iter().position(|p| p.name == o))
    {
        Some(i) => {
            if cfg.profiles.active == cfg.profiles.list[i].name {
                cfg.profiles.active = profile.name.clone();
            }
            cfg.profiles.list[i] = profile;
        }
        None => cfg.profiles.list.push(profile),
    }
    Ok(save(&app, &rt, cfg)?)
}

/// Saves the stack the configuration describes now as a new profile.
#[tauri::command]
pub fn profile_save_current(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> CommandResult<()> {
    let rt = state.runtime()?;
    let mut cfg = rt.config();
    let name = check_name(&cfg, &name, None)?;
    cfg.profiles.list.push(Profile::capture(&cfg, &name));
    cfg.profiles.active = name;
    Ok(save(&app, &rt, cfg)?)
}

#[tauri::command]
pub fn profile_delete(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> CommandResult<()> {
    let rt = state.runtime()?;
    let mut cfg = rt.config();
    cfg.profiles.list.retain(|p| p.name != name);
    Ok(save(&app, &rt, cfg)?)
}

/// Switches to the profile `name`: only changed services are restarted, and
/// a failure restores the previous stack (the config is then left as it was).
#[tauri::command]
pub async fn profile_switch(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> CommandResult<Plan> {
    let rt = state.runtime()?;
    Ok(switch(&app, &rt, &name).await?)
}

/// Shared with the tray.
pub async fn switch(app: &AppHandle, rt: &Runtime, name: &str) -> lumina_core::Result<Plan> {
    let old = rt.config();
    let profile = old
        .profiles
        .list
        .iter()
        .find(|p| p.name == name)
        .cloned()
        .ok_or_else(|| Error::Other(format!("there is no profile named \"{name}\"")))?;
    let missing = missing(rt, &profile).await;
    if !missing.is_empty() {
        return Err(Error::Other(format!(
            "{name} needs {} — install it in Quick Add first",
            missing.join(", ")
        )));
    }
    let mut new = old.clone();
    profile.apply(&mut new);
    let result = rt.stack.switch(&old, &new).await;
    if result.is_ok() {
        lumina_config::save(&rt.layout.config_file(), &new)?;
        *rt.config.write().unwrap_or_else(|e| e.into_inner()) = new;
        log::info!("switched to profile \"{name}\"");
        crate::commands::env_path::refresh_shims_later(app);
    }
    let _ = app.emit(events::CONFIG_CHANGED, "profiles");
    let _ = app.emit(events::SERVICE_STATUS, ());
    crate::tray::refresh(app.clone());
    result
}

/// The profiles that pin `version` of package `id` (for uninstall checks).
pub fn referencing(cfg: &AppConfig, id: &str, version: &str) -> Vec<String> {
    cfg.profiles
        .list
        .iter()
        .filter(|p| match id {
            "php" => p.php == version,
            "nginx" => p.web == WebServer::Nginx && p.web_version == version,
            "apache" => p.web == WebServer::Apache && p.web_version == version,
            "mysql" => p.database.engine == DatabaseEngine::Mysql && p.database.version == version,
            "mariadb" => {
                p.database.engine == DatabaseEngine::Mariadb && p.database.version == version
            }
            _ => false,
        })
        .map(|p| p.name.clone())
        .collect()
}

/// For the tray: (name, current) of every profile.
pub fn tray_entries(app: &AppHandle) -> Vec<(String, bool)> {
    let Ok(rt) = app.state::<AppState>().runtime() else {
        return Vec::new();
    };
    let cfg = rt.config();
    cfg.profiles
        .list
        .iter()
        .map(|p| (p.name.clone(), p.matches(&cfg)))
        .collect()
}
