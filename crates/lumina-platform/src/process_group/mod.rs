//! Kill-on-close process grouping: Job Objects on Windows, process groups + signals on Unix. Prevents ghost php-cgi processes.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
