//! Tauri shell: wires plugins, state, tray and IPC commands. Business logic lives
//! in `crates/*`; this crate stays a thin adapter.

mod commands;
mod error;
mod events;
mod notify;
mod setup;
mod state;
mod tray;

use tauri_plugin_log::{RotationStrategy, Target, TargetKind, TimezoneStrategy};

/// App log (`lumina.log`) goes to the OS log directory so it works before the
/// first-run wizard has chosen a data root. Service logs go to `<data-root>/logs`.
fn log_plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    let level = if cfg!(debug_assertions) {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    };
    tauri_plugin_log::Builder::new()
        .targets([
            Target::new(TargetKind::Stdout),
            Target::new(TargetKind::LogDir {
                file_name: Some("lumina".into()),
            }),
        ])
        .level(level)
        // Their debug output is per request (the ngrok agent API is polled
        // while a share opens) and drowns everything else.
        .level_for("hyper_util", log::LevelFilter::Info)
        .level_for("reqwest", log::LevelFilter::Info)
        .level_for("keyring", log::LevelFilter::Info)
        .max_file_size(5 * 1024 * 1024)
        .rotation_strategy(RotationStrategy::KeepSome(5))
        .timezone_strategy(TimezoneStrategy::UseLocal)
        .build()
}

pub fn run() {
    let app = tauri::Builder::default()
        // Must be registered first so a second launch focuses the existing window.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::show_main(app);
        }))
        .plugin(log_plugin())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            setup::init(app)?;
            tray::create(app.handle())?;
            setup::autostart(app.handle());
            Ok(())
        })
        .on_window_event(|window, event| {
            use tauri::Manager;
            if let tauri::WindowEvent::CloseRequested { api, .. } = event
                && setup::close_to_tray(window.app_handle())
            {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::app_version,
            commands::app::bootstrap_status,
            commands::app::bootstrap_complete,
            commands::app::set_language,
            commands::settings::settings_get,
            commands::settings::settings_update,
            commands::services::services_status,
            commands::services::service_start,
            commands::services::service_stop,
            commands::services::service_restart,
            commands::services::service_reload,
            commands::services::services_start_all,
            commands::services::services_stop_all,
            commands::services::php_versions,
            commands::services::web_versions,
            commands::services::db_versions,
            commands::services::redis_versions,
            commands::services::php_set_version,
            commands::services::service_open_log,
            commands::services::open_localhost,
            commands::services::open_www,
            commands::sites::sites_list,
            commands::sites::sites_sync,
            commands::sites::site_open,
            commands::sites::site_open_folder,
            commands::ssl::ssl_status,
            commands::ssl::ssl_set_enabled,
            commands::ssl::ssl_set_force_https,
            commands::ssl::ssl_trust,
            commands::ssl::ssl_regenerate,
            commands::ssl::ssl_remove_ca,
            commands::packages::packages_list,
            commands::packages::package_install,
            commands::packages::package_cancel,
            commands::packages::package_dismiss,
            commands::packages::package_uninstall,
            commands::packages::package_set_active,
            commands::packages::db_ui_open,
            commands::profiles::profiles_list,
            commands::profiles::profile_save,
            commands::profiles::profile_save_current,
            commands::profiles::profile_delete,
            commands::profiles::profile_switch,
            commands::env_path::path_status,
            commands::env_path::path_add,
            commands::env_path::path_remove,
            commands::env_path::path_add_system,
            commands::env_path::terminal_open,
            commands::editor::editor_presets,
            commands::editor::editor_open,
            commands::quickapp::quickapp_overview,
            commands::quickapp::quickapp_create,
            commands::quickapp::quickapp_cancel,
            commands::quickapp::quickapp_dismiss,
            commands::quickapp::quickapp_delete_partial,
            commands::tunnel::tunnel_overview,
            commands::tunnel::tunnel_set_token,
            commands::tunnel::tunnel_clear_token,
            commands::tunnel::tunnel_set_domain,
            commands::tunnel::tunnel_start,
            commands::tunnel::tunnel_stop,
            commands::tunnel::tunnel_stop_all,
            commands::tunnel::tunnel_open_log,
            commands::logs::logs_list,
            commands::logs::logs_read,
            commands::logs::logs_open,
            commands::logs::logs_open_folder,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Lumina");

    app.run(|app, event| {
        if let tauri::RunEvent::Exit = event {
            stop_services(app);
        }
    });
}

/// Stops every service before the process exits (web servers first, nginx
/// gracefully). Windows would kill them anyway (Job Objects), but a clean stop
/// lets servers finish requests and keeps the PID ledger empty.
fn stop_services(app: &tauri::AppHandle) {
    use tauri::Manager;
    let Some(state) = app.try_state::<state::AppState>() else {
        return;
    };
    let Ok(runtime) = state.runtime() else {
        return;
    };
    log::info!("stopping services before exit");
    // Tunnels first: they forward to the web server.
    if let Err(err) =
        tauri::async_runtime::block_on(lumina_tunnel::TunnelProvider::shutdown(&runtime.tunnels))
    {
        log::error!("could not stop the ngrok agent on exit: {err}");
    }
    let failures = tauri::async_runtime::block_on(runtime.stack.stop_all());
    for (kind, err) in failures {
        log::error!("could not stop {kind:?} on exit: {err}");
    }
}
