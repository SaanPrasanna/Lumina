//! Add/remove bundled PHP, Node, Python, Composer to the user PATH (registry on Windows, shell rc files on macOS/Linux).

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
