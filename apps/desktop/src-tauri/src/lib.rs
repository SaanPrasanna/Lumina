//! Tauri shell: wires plugins, state, tray and IPC commands. Business logic lives
//! in `crates/*`; this crate stays a thin adapter.

mod commands;
mod error;
mod events;
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
        .max_file_size(5 * 1024 * 1024)
        .rotation_strategy(RotationStrategy::KeepSome(5))
        .timezone_strategy(TimezoneStrategy::UseLocal)
        .build()
}

pub fn run() {
    tauri::Builder::default()
        // Must be registered first so a second launch focuses the existing window.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            use tauri::Manager;
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(log_plugin())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(setup::init)
        .invoke_handler(tauri::generate_handler![
            commands::app::app_version,
            commands::app::bootstrap_status,
            commands::app::bootstrap_complete,
            commands::app::set_language,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Lumina");
}
