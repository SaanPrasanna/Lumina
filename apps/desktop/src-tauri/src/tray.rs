//! System tray: Start/Stop All, per-service toggles, quick links, Quit.
//!
//! The menu is rebuilt whenever a service changes state, so the check marks
//! always match reality. Labels follow the UI language (English or Sinhala).

use lumina_core::types::ServiceKind;
use lumina_services::orchestrator::Orchestrator;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Wry};

use crate::events;
use crate::state::AppState;

const TRAY_ID: &str = "lumina";

struct Labels {
    start_all: &'static str,
    stop_all: &'static str,
    web: &'static str,
    database: &'static str,
    open_localhost: &'static str,
    open_www: &'static str,
    show: &'static str,
    quit: &'static str,
}

fn labels(language: &str) -> Labels {
    match language {
        "si" => Labels {
            start_all: "සියල්ල ආරම්භ කරන්න",
            stop_all: "සියල්ල නවත්වන්න",
            web: "වෙබ් සේවාදායකය",
            database: "දත්ත සමුදාය",
            open_localhost: "localhost විවෘත කරන්න",
            open_www: "www ෆෝල්ඩරය විවෘත කරන්න",
            show: "Lumina පෙන්වන්න",
            quit: "ඉවත් වන්න",
        },
        _ => Labels {
            start_all: "Start All",
            stop_all: "Stop All",
            web: "Web server",
            database: "Database",
            open_localhost: "Open localhost",
            open_www: "Open www folder",
            show: "Show Lumina",
            quit: "Quit",
        },
    }
}

/// Current language, from the bootstrap file.
fn language(app: &AppHandle) -> String {
    app.state::<AppState>()
        .bootstrap
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .map_or_else(|| "en".to_owned(), |b| b.language.clone())
}

/// What the tray shows for each toggle.
struct States {
    php: bool,
    web: bool,
    web_name: &'static str,
    db: bool,
    db_name: &'static str,
}

async fn service_states(app: &AppHandle) -> States {
    let Ok(rt) = app.state::<AppState>().runtime() else {
        return States {
            php: false,
            web: false,
            web_name: "nginx",
            db: false,
            db_name: "MySQL",
        };
    };
    let cfg = rt.config();
    let status = rt.stack.status(&cfg).await;
    let web_name = match Orchestrator::configured_web(&cfg) {
        ServiceKind::Apache => "Apache",
        _ => "nginx",
    };
    let db_name = match Orchestrator::configured_db(&cfg) {
        ServiceKind::Mariadb => "MariaDB",
        _ => "MySQL",
    };
    States {
        php: status.php.runtime.is_some(),
        web: status.web.runtime.is_some(),
        web_name,
        db: status.database.runtime.is_some(),
        db_name,
    }
}

fn build_menu(app: &AppHandle, language: &str, s: &States) -> tauri::Result<Menu<Wry>> {
    let l = labels(language);
    let configured = app.state::<AppState>().runtime().is_ok();
    Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "start-all", l.start_all, configured, None::<&str>)?,
            &MenuItem::with_id(app, "stop-all", l.stop_all, configured, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &CheckMenuItem::with_id(app, "toggle-php", "PHP", configured, s.php, None::<&str>)?,
            &CheckMenuItem::with_id(
                app,
                "toggle-web",
                format!("{} ({})", l.web, s.web_name),
                configured,
                s.web,
                None::<&str>,
            )?,
            &CheckMenuItem::with_id(
                app,
                "toggle-db",
                format!("{} ({})", l.database, s.db_name),
                configured,
                s.db,
                None::<&str>,
            )?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(
                app,
                "open-localhost",
                l.open_localhost,
                configured,
                None::<&str>,
            )?,
            &MenuItem::with_id(app, "open-www", l.open_www, configured, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "show", l.show, true, None::<&str>)?,
            &MenuItem::with_id(app, "quit", l.quit, true, None::<&str>)?,
        ],
    )
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let initial = States {
        php: false,
        web: false,
        web_name: "nginx",
        db: false,
        db_name: "MySQL",
    };
    let menu = build_menu(app, &language(app), &initial)?;
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
    refresh(app.clone());
    Ok(())
}

/// Rebuilds the menu from the current service states and language.
pub fn refresh(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let states = service_states(&app).await;
        let Some(tray) = app.tray_by_id(TRAY_ID) else {
            return;
        };
        match build_menu(&app, &language(&app), &states) {
            Ok(menu) => {
                let _ = tray.set_menu(Some(menu));
            }
            Err(err) => log::error!("tray menu: {err}"),
        }
    });
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
        "open-localhost" => report(app, crate::commands::services::open_localhost_for(app)),
        "open-www" => report(app, crate::commands::services::open_www_for(app)),
        "start-all" | "stop-all" | "toggle-php" | "toggle-web" | "toggle-db" => {
            let app = app.clone();
            let id = id.to_owned();
            tauri::async_runtime::spawn(async move {
                let result = run_service_action(&app, &id).await;
                if let Err(err) = &result {
                    // Errors are shown on the Services page.
                    log::warn!("tray action `{id}` failed: {err}");
                    show_main(&app);
                    let _ = app.emit("navigate", "/services");
                }
                let _ = app.emit(events::SERVICE_STATUS, ());
                refresh(app);
            });
        }
        _ => {}
    }
}

async fn run_service_action(app: &AppHandle, id: &str) -> lumina_core::Result<()> {
    let rt = app
        .state::<AppState>()
        .runtime()
        .map_err(|e| lumina_core::Error::Other(e.to_string()))?;
    let cfg = rt.config();
    let web = Orchestrator::configured_web(&cfg);
    let db = Orchestrator::configured_db(&cfg);
    let status = rt.stack.status(&cfg).await;
    match id {
        "start-all" => rt.stack.start_all(&cfg).await,
        "stop-all" => match rt.stack.stop_all().await.into_iter().next() {
            Some((_, err)) => Err(err),
            None => Ok(()),
        },
        "toggle-php" if status.php.runtime.is_some() => rt.stack.stop(ServiceKind::PhpFcgi).await,
        "toggle-php" => rt.stack.start(ServiceKind::PhpFcgi, &cfg).await,
        "toggle-web" if status.web.runtime.is_some() => rt.stack.stop(web).await,
        "toggle-web" => rt.stack.start(web, &cfg).await,
        "toggle-db" if status.database.runtime.is_some() => rt.stack.stop(db).await,
        "toggle-db" => rt.stack.start(db, &cfg).await,
        _ => Ok(()),
    }
}

fn report(app: &AppHandle, result: crate::error::CommandResult<()>) {
    if let Err(err) = result {
        log::warn!("tray: {}", err.message);
        show_main(app);
    }
}
