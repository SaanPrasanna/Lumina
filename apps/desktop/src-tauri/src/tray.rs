//! System tray (and the window's right-click menu), laid out like a classic local-server panel: one submenu per
//! service (start/stop, version, config file, log), the projects under www,
//! shortcuts into the app, and Start/Stop All.
//!
//! The menu is rebuilt whenever a service, the config, the projects or the
//! installed packages change, so check marks and versions always match
//! reality. A running service carries a green dot. Labels follow the UI
//! language (English or Sinhala).

use lumina_config::schema::DatabaseEngine;
use lumina_core::types::ServiceKind;
use lumina_services::install::Install;
use lumina_services::orchestrator::Orchestrator;
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Listener, Manager, Wry};
use tauri_plugin_opener::OpenerExt;

use crate::commands::editor::{self, ConfigFile, OpenTarget};
use crate::commands::{env_path, packages, services, sites};
use crate::error::CommandResult;
use crate::events;
use crate::state::{AppState, Runtime};

const TRAY_ID: &str = "lumina";
/// Projects listed under www; the Sites page has the rest.
const MAX_SITES: usize = 25;

struct Labels {
    show: &'static str,
    settings: &'static str,
    profiles: &'static str,
    open_localhost: &'static str,
    open_www: &'static str,
    no_sites: &'static str,
    quick_app: &'static str,
    tools: &'static str,
    terminal: &'static str,
    packages: &'static str,
    sharing: &'static str,
    path: &'static str,
    logs: &'static str,
    start: &'static str,
    stop: &'static str,
    version: &'static str,
    newest: &'static str,
    not_installed: &'static str,
    install: &'static str,
    open_log: &'static str,
    ext_dir: &'static str,
    sites_config: &'static str,
    inbox: &'static str,
    start_all: &'static str,
    stop_all: &'static str,
    quit: &'static str,
}

fn labels(language: &str) -> Labels {
    match language {
        "si" => Labels {
            show: "Lumina පෙන්වන්න",
            settings: "සැකසීම්…",
            profiles: "පැතිකඩ",
            open_localhost: "localhost විවෘත කරන්න",
            open_www: "www ෆෝල්ඩරය විවෘත කරන්න",
            no_sites: "තවම ව්‍යාපෘති නැත",
            quick_app: "QuickApp…",
            tools: "මෙවලම්",
            terminal: "ටර්මිනලය විවෘත කරන්න",
            packages: "ඉක්මන් එකතුව…",
            sharing: "බෙදාගැනීම…",
            path: "PATH…",
            logs: "ලොග්…",
            start: "ආරම්භ කරන්න",
            stop: "නවත්වන්න",
            version: "සංස්කරණය",
            newest: "නවතම ස්ථාපිත",
            not_installed: "ස්ථාපනය කර නැත",
            install: "ඉක්මන් එකතුව වෙතින් ස්ථාපනය කරන්න…",
            open_log: "ලොගය විවෘත කරන්න",
            ext_dir: "දිගු ෆෝල්ඩරය",
            sites_config: "අඩවි වින්‍යාස ෆෝල්ඩරය",
            inbox: "එන ලිපි විවෘත කරන්න",
            start_all: "සියල්ල ආරම්භ කරන්න",
            stop_all: "සියල්ල නවත්වන්න",
            quit: "ඉවත් වන්න",
        },
        _ => Labels {
            show: "Show Lumina",
            settings: "Settings…",
            profiles: "Profile",
            open_localhost: "Open localhost",
            open_www: "Open www folder",
            no_sites: "No projects yet",
            quick_app: "QuickApp…",
            tools: "Tools",
            terminal: "Open terminal",
            packages: "Quick Add…",
            sharing: "Sharing…",
            path: "PATH…",
            logs: "Logs…",
            start: "Start",
            stop: "Stop",
            version: "Version",
            newest: "Newest installed",
            not_installed: "Not installed",
            install: "Install from Quick Add…",
            open_log: "Open log",
            ext_dir: "Extensions folder",
            sites_config: "Sites config folder",
            inbox: "Open inbox",
            start_all: "Start All",
            stop_all: "Stop All",
            quit: "Quit",
        },
    }
}

/// Current language, from the bootstrap file.
pub(crate) fn language(app: &AppHandle) -> String {
    app.state::<AppState>()
        .bootstrap
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .map_or_else(|| "en".to_owned(), |b| b.language.clone())
}

/// One service or tool submenu.
struct Row {
    /// `None` for tools (Node.js, Python): versions only.
    kind: Option<ServiceKind>,
    /// Catalog ID, which also names the config field holding the version.
    package: &'static str,
    name: &'static str,
    /// Installed versions, newest first.
    versions: Vec<String>,
    /// The configured version ("" = newest).
    wanted: String,
    /// The version that runs, if it does.
    running: Option<String>,
}

impl Row {
    /// The version that runs, or else the one that would.
    fn current(&self) -> Option<&str> {
        self.running
            .as_deref()
            .or_else(|| packages::pick(&self.versions, &self.wanted).map(String::as_str))
    }
}

/// Everything the menu shows.
#[derive(Default)]
struct Snapshot {
    configured: bool,
    services: Vec<Row>,
    tools: Vec<Row>,
    /// Domains of the projects under www.
    sites: Vec<String>,
    /// (name, current) of every saved profile.
    profiles: Vec<(String, bool)>,
}

fn names(installs: Vec<Install>) -> Vec<String> {
    installs.into_iter().map(|i| i.version).collect()
}

async fn snapshot(app: &AppHandle) -> Snapshot {
    let Ok(rt) = app.state::<AppState>().runtime() else {
        return Snapshot::default();
    };
    let cfg = rt.config();
    let stack = &rt.stack;
    let status = stack.status(&cfg).await;
    let row = |kind, package, name, versions, running| Row {
        kind,
        package,
        name,
        versions,
        wanted: packages::selected(&cfg, package).unwrap_or_default(),
        running,
    };

    let (web_package, web_name, web_versions) = match Orchestrator::configured_web(&cfg) {
        ServiceKind::Apache => ("apache", "Apache", stack.apache().versions().await),
        _ => ("nginx", "nginx", stack.nginx().versions().await),
    };
    let (engine, db_package, db_name) = match cfg.database.engine {
        DatabaseEngine::Mysql => (DatabaseEngine::Mysql, "mysql", "MySQL"),
        DatabaseEngine::Mariadb => (DatabaseEngine::Mariadb, "mariadb", "MariaDB"),
    };
    let services = vec![
        row(
            Some(ServiceKind::PhpFcgi),
            "php",
            "PHP",
            names(stack.php().versions().await),
            status.php.runtime.map(|r| r.version),
        ),
        row(
            Some(Orchestrator::configured_web(&cfg)),
            web_package,
            web_name,
            names(web_versions),
            status.web.runtime.map(|r| r.version),
        ),
        row(
            Some(Orchestrator::configured_db(&cfg)),
            db_package,
            db_name,
            names(stack.database(engine).versions().await),
            status.database.runtime.map(|r| r.version),
        ),
        row(
            Some(ServiceKind::Postgresql),
            "postgresql",
            "PostgreSQL",
            names(stack.postgresql().versions().await),
            status.postgresql.runtime.map(|r| r.version),
        ),
        row(
            Some(ServiceKind::Redis),
            "redis",
            "Redis",
            names(stack.redis().versions().await),
            status.redis.runtime.map(|r| r.version),
        ),
        row(
            Some(ServiceKind::Mailpit),
            "mailpit",
            "Mailpit",
            names(stack.mailpit().versions().await),
            status.mailpit.runtime.map(|r| r.version),
        ),
    ];
    let tool = |package, name| {
        let mut versions: Vec<String> = rt
            .packages
            .installed(package)
            .into_iter()
            .map(|i| i.version)
            .collect();
        versions.sort_by_key(|v| std::cmp::Reverse(version_key(v)));
        row(None, package, name, versions, None)
    };
    Snapshot {
        configured: true,
        services,
        tools: vec![
            tool("node", "Node.js"),
            tool("python", "Python"),
            tool("bun", "Bun"),
            tool("deno", "Deno"),
        ],
        sites: stack
            .sites(&cfg)
            .sites
            .into_iter()
            .map(|s| s.domain)
            .take(MAX_SITES)
            .collect(),
        profiles: crate::commands::profiles::tray_entries(app),
    }
}

/// "24.21.0" → [24, 21, 0], for sorting newest first.
fn version_key(version: &str) -> Vec<u64> {
    version
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|p| p.parse().ok())
        .collect()
}

/// The ID a service has in menu item IDs (its serde name, e.g. `php-fcgi`).
fn kind_id(kind: ServiceKind) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn parse_kind(id: &str) -> Option<ServiceKind> {
    serde_json::from_value(serde_json::Value::String(id.to_owned())).ok()
}

/// A small green dot: the service runs.
fn running_dot() -> Image<'static> {
    const SIZE: u32 = 16;
    const RADIUS: f32 = 4.0;
    let centre = (SIZE as f32 - 1.0) / 2.0;
    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let distance = (x as f32 - centre).hypot(y as f32 - centre);
            // One pixel of anti-aliasing at the edge.
            let alpha = (RADIUS + 0.5 - distance).clamp(0.0, 1.0);
            rgba.extend_from_slice(&[0x2e, 0x9e, 0x6e, (alpha * 255.0).round() as u8]);
        }
    }
    Image::new_owned(rgba, SIZE, SIZE)
}

type Items = Vec<Box<dyn IsMenuItem<Wry>>>;

fn submenu(
    app: &AppHandle,
    text: &str,
    enabled: bool,
    icon: Option<Image<'_>>,
    items: &Items,
) -> tauri::Result<Submenu<Wry>> {
    let menu = Submenu::with_id_and_icon(app, format!("menu:{text}"), text, enabled, icon)?;
    let refs: Vec<&dyn IsMenuItem<Wry>> = items.iter().map(AsRef::as_ref).collect();
    menu.append_items(&refs)?;
    Ok(menu)
}

fn item(
    app: &AppHandle,
    id: impl AsRef<str>,
    text: &str,
    enabled: bool,
) -> tauri::Result<Box<dyn IsMenuItem<Wry>>> {
    Ok(Box::new(MenuItem::with_id(
        app,
        id.as_ref(),
        text,
        enabled,
        None::<&str>,
    )?))
}

fn check(
    app: &AppHandle,
    id: impl AsRef<str>,
    text: &str,
    enabled: bool,
    checked: bool,
) -> tauri::Result<Box<dyn IsMenuItem<Wry>>> {
    Ok(Box::new(CheckMenuItem::with_id(
        app,
        id.as_ref(),
        text,
        enabled,
        checked,
        None::<&str>,
    )?))
}

fn separator(app: &AppHandle) -> tauri::Result<Box<dyn IsMenuItem<Wry>>> {
    Ok(Box::new(PredefinedMenuItem::separator(app)?))
}

/// Newest installed / 8.4.26 / 8.3.30 …; picking one saves it and restarts
/// the service if it runs.
fn version_items(app: &AppHandle, l: &Labels, row: &Row) -> tauri::Result<Items> {
    let mut items: Items = Vec::new();
    if row.versions.is_empty() {
        items.push(item(app, "none", l.not_installed, false)?);
        items.push(item(app, "nav:/packages", l.install, true)?);
        return Ok(items);
    }
    let newest = row.wanted.trim().is_empty();
    items.push(check(
        app,
        format!("ver:{}:", row.package),
        &format!("{} ({})", l.newest, row.versions[0]),
        true,
        newest,
    )?);
    items.push(separator(app)?);
    let picked = packages::pick(&row.versions, &row.wanted);
    for version in &row.versions {
        items.push(check(
            app,
            format!("ver:{}:{version}", row.package),
            version,
            true,
            !newest && picked == Some(version),
        )?);
    }
    Ok(items)
}

/// PHP ▸ Start/Stop, Version, php.ini, extensions, log (and so on per
/// service); a tool (Node.js, Python) ▸ its versions.
fn service_menu(
    app: &AppHandle,
    l: &Labels,
    row: &Row,
    enabled: bool,
) -> tauri::Result<Submenu<Wry>> {
    let Some(kind) = row.kind else {
        return submenu(app, row.name, enabled, None, &version_items(app, l, row)?);
    };
    let id = kind_id(kind);
    let installed = !row.versions.is_empty();
    let mut items: Items = Vec::new();
    if row.running.is_some() {
        items.push(item(app, format!("stop:{id}"), l.stop, true)?);
    } else {
        items.push(item(app, format!("start:{id}"), l.start, installed)?);
    }
    let version_text = match row.current() {
        Some(v) => format!("{} ({v})", l.version),
        None => l.version.to_owned(),
    };
    items.push(Box::new(submenu(
        app,
        &version_text,
        true,
        None,
        &version_items(app, l, row)?,
    )?));
    items.push(separator(app)?);
    match kind {
        ServiceKind::PhpFcgi => {
            items.push(item(app, "edit:php", "php.ini", installed)?);
            items.push(item(app, "php-ext", l.ext_dir, installed)?);
        }
        ServiceKind::Nginx | ServiceKind::Apache => {
            items.push(item(app, "edit:web", l.sites_config, true)?);
        }
        ServiceKind::Mysql | ServiceKind::Mariadb => {
            items.push(item(app, "edit:database", "custom.cnf", true)?);
            items.push(item(app, "db-ui", "phpMyAdmin", row.running.is_some())?);
        }
        ServiceKind::Postgresql => {
            items.push(item(app, "edit:postgresql", "postgresql.conf", installed)?);
        }
        ServiceKind::Redis => {
            items.push(item(app, "edit:redis", "custom.conf", true)?);
        }
        ServiceKind::Mailpit => {
            items.push(item(app, "mail-inbox", l.inbox, row.running.is_some())?);
        }
    }
    items.push(item(app, format!("log:{id}"), l.open_log, true)?);
    let icon = row.running.as_ref().map(|_| running_dot());
    submenu(app, row.name, enabled, icon, &items)
}

/// Where the menu opens: the tray, or right-clicking inside the window (which
/// is already shown, so "Show Lumina" is left out).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Place {
    Tray,
    Window,
}

fn build_menu(
    app: &AppHandle,
    language: &str,
    s: &Snapshot,
    place: Place,
) -> tauri::Result<Menu<Wry>> {
    let l = labels(language);
    let on = s.configured;

    // Lumina ▸ Show, Settings, Profile ▸
    let mut profiles: Items = Vec::new();
    for (i, (name, current)) in s.profiles.iter().enumerate() {
        profiles.push(check(app, format!("profile:{i}"), name, on, *current)?);
    }
    let mut lumina: Items = Vec::new();
    if place == Place::Tray {
        lumina.push(item(app, "show", l.show, true)?);
    }
    lumina.extend([
        item(app, "nav:/settings", l.settings, on)?,
        separator(app)?,
        Box::new(submenu(
            app,
            l.profiles,
            !profiles.is_empty(),
            None,
            &profiles,
        )?),
    ]);

    // www ▸ localhost, folder, every project
    let mut www: Items = vec![
        item(app, "open-localhost", l.open_localhost, on)?,
        item(app, "open-www", l.open_www, on)?,
        separator(app)?,
    ];
    if s.sites.is_empty() {
        www.push(item(app, "none", l.no_sites, false)?);
    }
    for domain in &s.sites {
        www.push(item(app, format!("site:{domain}"), domain, on)?);
    }

    let tools: Items = vec![
        item(app, "terminal", l.terminal, on)?,
        separator(app)?,
        item(app, "nav:/packages", l.packages, on)?,
        item(app, "nav:/tunnel", l.sharing, on)?,
        item(app, "nav:/env-path", l.path, on)?,
        item(app, "nav:/logs", l.logs, on)?,
    ];

    let mut top: Items = vec![
        Box::new(submenu(app, "Lumina", true, None, &lumina)?),
        Box::new(submenu(app, "www", on, None, &www)?),
        item(app, "nav:/quickapp", l.quick_app, on)?,
        Box::new(submenu(app, l.tools, on, None, &tools)?),
        separator(app)?,
    ];
    for row in &s.services {
        top.push(Box::new(service_menu(app, &l, row, on)?));
    }
    top.push(separator(app)?);
    for row in &s.tools {
        top.push(Box::new(service_menu(app, &l, row, on)?));
    }
    top.extend([
        separator(app)?,
        item(app, "start-all", l.start_all, on)?,
        item(app, "stop-all", l.stop_all, on)?,
        separator(app)?,
        item(app, "quit", l.quit, true)?,
    ]);
    let refs: Vec<&dyn IsMenuItem<Wry>> = top.iter().map(AsRef::as_ref).collect();
    Menu::with_items(app, &refs)
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app, &language(app), &Snapshot::default(), Place::Tray)?;
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Lumina")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| on_menu(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    // Versions, projects and settings show in the menu too.
    for event in [
        events::CONFIG_CHANGED,
        events::PACKAGES_CHANGED,
        events::SITES_CHANGED,
    ] {
        let app = app.clone();
        app.clone().listen_any(event, move |_| refresh(app.clone()));
    }
    refresh(app.clone());
    Ok(())
}

/// Rebuilds the menu from the current state and language.
pub fn refresh(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let snapshot = snapshot(&app).await;
        let Some(tray) = app.tray_by_id(TRAY_ID) else {
            return;
        };
        match build_menu(&app, &language(&app), &snapshot, Place::Tray) {
            Ok(menu) => {
                let _ = tray.set_menu(Some(menu));
            }
            Err(err) => log::error!("tray menu: {err}"),
        }
    });
}

/// Opens the same menu at the cursor inside `window` (right-click). Its
/// clicks arrive in [`on_menu`] like the tray's.
pub async fn popup(app: &AppHandle, window: &tauri::Window) -> tauri::Result<()> {
    let snapshot = snapshot(app).await;
    let menu = build_menu(app, &language(app), &snapshot, Place::Window)?;
    window.popup_menu(&menu)
}

pub fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn on_menu(app: &AppHandle, id: &str) {
    match id {
        "show" => show_main(app),
        "quit" => app.exit(0), // RunEvent::Exit stops every service
        id if id.starts_with("nav:") => {
            show_main(app);
            let _ = app.emit("navigate", &id["nav:".len()..]);
        }
        id => {
            let app = app.clone();
            let id = id.to_owned();
            tauri::async_runtime::spawn(async move {
                let Ok(rt) = app.state::<AppState>().runtime() else {
                    return;
                };
                if let Err(err) = run(&app, &rt, &id).await {
                    log::warn!("tray action `{id}` failed: {}", err.message);
                    crate::notify::send(
                        &app,
                        crate::notify::Note::ActionFailed {
                            message: &err.message,
                        },
                    );
                }
                let _ = app.emit(events::SERVICE_STATUS, ());
                refresh(app);
            });
        }
    }
}

async fn run(app: &AppHandle, rt: &std::sync::Arc<Runtime>, id: &str) -> CommandResult<()> {
    let cfg = rt.config();
    let (action, arg) = id.split_once(':').unwrap_or((id, ""));
    match action {
        "start-all" => rt.stack.start_all(&cfg).await?,
        "stop-all" => {
            if let Some((_, err)) = rt.stack.stop_all().await.into_iter().next() {
                return Err(err.into());
            }
        }
        "start" | "stop" => {
            let Some(kind) = parse_kind(arg) else {
                return Ok(());
            };
            if action == "start" {
                rt.stack.start(kind, &cfg).await?;
            } else {
                rt.stack.stop(kind).await?;
            }
        }
        "ver" => {
            let (package, version) = arg.split_once(':').unwrap_or((arg, ""));
            packages::set_active(app, rt, package, version.to_owned()).await?;
        }
        "edit" => {
            let file = match arg {
                "php" => ConfigFile::PhpIni,
                "web" => ConfigFile::WebSites,
                "database" => ConfigFile::Database,
                "postgresql" => ConfigFile::Postgresql,
                "redis" => ConfigFile::Redis,
                _ => return Ok(()),
            };
            editor::open(rt, &OpenTarget::Config { file }, None).await?;
        }
        "log" => {
            if let Some(kind) = parse_kind(arg) {
                services::open_log_for(app, rt, kind)?;
            }
        }
        "php-ext" => {
            let installs = rt.stack.php().versions().await;
            if let Some(php) = lumina_services::install::select(&installs, &cfg.php.version) {
                open_folder(app, &php.dir.join("ext"))?;
            }
        }
        "db-ui" => packages::open_db_ui(app, rt).await?,
        "mail-inbox" => services::mailpit_open_for(app)?,
        "site" => sites::open_site_for(app, rt, arg)?,
        "open-localhost" => services::open_localhost_for(app)?,
        "open-www" => services::open_www_for(app)?,
        "terminal" => env_path::open_terminal_for(rt, None).await?,
        "profile" => {
            let name = arg
                .parse::<usize>()
                .ok()
                .and_then(|i| cfg.profiles.list.get(i).map(|p| p.name.clone()));
            if let Some(name) = name
                && let Err(err) = crate::commands::profiles::switch(app, rt, &name).await
            {
                show_main(app);
                let _ = app.emit("navigate", "/profiles");
                let _ = app.emit(events::PROFILE_SWITCH_FAILED, err.to_string());
            }
        }
        _ => {}
    }
    Ok(())
}

fn open_folder(app: &AppHandle, dir: &std::path::Path) -> CommandResult<()> {
    app.opener()
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(|e| lumina_core::Error::Other(e.to_string()).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_ids_round_trip() {
        for kind in [
            ServiceKind::PhpFcgi,
            ServiceKind::Nginx,
            ServiceKind::Apache,
            ServiceKind::Mysql,
            ServiceKind::Mariadb,
            ServiceKind::Postgresql,
            ServiceKind::Redis,
            ServiceKind::Mailpit,
        ] {
            assert_eq!(parse_kind(&kind_id(kind)), Some(kind));
        }
    }

    #[test]
    fn tool_versions_sort_newest_first() {
        let mut v = vec![
            "22.9.0".to_owned(),
            "24.21.0".to_owned(),
            "22.10.1".to_owned(),
        ];
        v.sort_by_key(|v| std::cmp::Reverse(version_key(v)));
        assert_eq!(v, ["24.21.0", "22.10.1", "22.9.0"]);
    }
}
