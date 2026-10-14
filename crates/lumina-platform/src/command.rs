//! Helpers for short-lived helper commands (e.g. `php-cgi -v`, `netstat`).

use std::process::Command;

/// Prevents a console window from flashing up when a GUI app runs a console
/// program on Windows. No-op elsewhere.
pub fn hide_console(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = cmd;
}
