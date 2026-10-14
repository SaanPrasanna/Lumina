//! Elevated helper. The GUI never runs as admin; it launches this binary with
//! elevation for a single, validated operation (hosts edit, CA install, system PATH)
//! and exits. Keep its surface minimal and audited.
//!
//! The hosts file path is fixed here on purpose: neither arguments nor the
//! environment can point the helper at another file.

// No console window when started elevated from the GUI on Windows.
#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let hosts_file = lumina_platform::hosts::system_hosts_path();
    match lumina_helper::run(&args, &hosts_file) {
        Ok(()) => std::process::exit(lumina_helper::exit::OK),
        Err(err) => {
            eprintln!("lumina-helper: {err}");
            std::process::exit(err.exit_code());
        }
    }
}
