//! Tauri shell: wires plugins, state, tray and IPC commands. Business logic lives
//! in `crates/*`; this crate stays a thin adapter.

mod commands;
mod error;
mod events;
mod setup;
mod state;
mod tray;

pub fn run() {
    tauri::Builder::default()
        // Must be registered first so a second launch focuses the existing window.
        .plugin(tauri_plugin_single_instance::init(|_app, _args, _cwd| {}))
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(setup::init)
        .invoke_handler(tauri::generate_handler![commands::app::app_version])
        .run(tauri::generate_context!())
        .expect("error while running Lumina");
}
