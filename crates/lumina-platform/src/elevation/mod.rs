//! Run privileged operations: UAC (runas) on Windows, authorization prompt on macOS, pkexec on Linux.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
