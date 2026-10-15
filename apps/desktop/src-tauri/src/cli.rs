//! Command-line flags. A second launch hands its flags to the running Lumina
//! (single instance), so they work whether or not Lumina is already open:
//!
//! - `--minimized`: start in the tray without showing the window (used by
//!   "Start Lumina when I sign in").
//! - `--stop-all`: stop every service; Lumina keeps running.
//! - `--quit`: stop every service and exit (the uninstaller runs this so no
//!   file is in use).
//! - `--start-profile <name>`: switch to a saved profile.
//! - `--remove-data`: delete the data root and forget it (the uninstaller runs
//!   this only after the user said yes; `Layout::remove` refuses anything that
//!   is not a Lumina data folder).

use tauri::{AppHandle, Emitter, Manager};

use crate::state::AppState;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Flags {
    pub minimized: bool,
    pub stop_all: bool,
    pub quit: bool,
    pub start_profile: Option<String>,
    pub remove_data: bool,
}

impl Flags {
    /// Parses the arguments after the program name. Unknown arguments are
    /// ignored (the OS may add some, e.g. macOS `-psn_…`).
    pub fn parse<I, S>(args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut flags = Self::default();
        let mut args = args.into_iter().map(|a| a.as_ref().to_owned());
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--minimized" => flags.minimized = true,
                "--stop-all" => flags.stop_all = true,
                "--quit" => flags.quit = true,
                "--remove-data" => flags.remove_data = true,
                "--start-profile" => flags.start_profile = args.next().filter(|n| !n.is_empty()),
                other => {
                    if let Some(name) = other.strip_prefix("--start-profile=") {
                        flags.start_profile = Some(name.to_owned()).filter(|n| !n.is_empty());
                    }
                }
            }
        }
        flags
    }

    /// Only asks to stop things: a first launch with these flags has nothing
    /// to stop and exits without opening a window.
    pub fn only_stops(&self) -> bool {
        (self.stop_all || self.quit || self.remove_data) && self.start_profile.is_none()
    }

    /// Whether the window should appear.
    pub fn shows_window(&self) -> bool {
        !self.minimized && !self.only_stops()
    }
}

/// `--remove-data`: deletes the data root named in `bootstrap.toml`, then the
/// bootstrap file, so the next start shows the first-run wizard.
pub fn remove_data(bootstrap_path: &std::path::Path) {
    let Ok(Some(b)) = lumina_config::bootstrap::load(bootstrap_path) else {
        log::info!("--remove-data: no data folder is recorded");
        return;
    };
    match lumina_core::paths::Layout::new(&b.data_root).remove() {
        Ok(()) => {
            log::info!("--remove-data: deleted {}", b.data_root.display());
            let _ = std::fs::remove_file(bootstrap_path);
        }
        Err(err) => log::error!("--remove-data: {err}"),
    }
}

/// Carries out `flags` in the running app (first launch, after setup, or a
/// second launch forwarded by the single-instance plugin).
pub fn apply(app: &AppHandle, flags: Flags) {
    if flags.quit {
        log::info!("--quit: stopping services and exiting");
        app.exit(0); // RunEvent::Exit stops every service
        return;
    }
    if flags.stop_all {
        log::info!("--stop-all: stopping every service");
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Ok(rt) = app.state::<AppState>().runtime() {
                let _ = lumina_tunnel::TunnelProvider::shutdown(&rt.tunnels).await;
                for (kind, err) in rt.stack.stop_all().await {
                    log::error!("could not stop {kind:?}: {err}");
                }
            }
        });
    }
    let shows_window = flags.shows_window();
    if let Some(name) = flags.start_profile {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let Ok(rt) = app.state::<AppState>().runtime() else {
                log::warn!("--start-profile {name}: Lumina is not set up yet");
                return;
            };
            log::info!("--start-profile: switching to {name}");
            if let Err(err) = crate::commands::profiles::switch(&app, &rt, &name).await {
                log::error!("could not switch to profile {name}: {err}");
                let _ = app.emit(crate::events::PROFILE_SWITCH_FAILED, err.to_string());
            }
        });
    }
    if shows_window {
        crate::tray::show_main(app);
    }
}
