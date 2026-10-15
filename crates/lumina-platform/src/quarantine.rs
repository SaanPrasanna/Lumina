//! macOS Gatekeeper quarantine.
//!
//! Files written by a quarantine-aware process carry the
//! `com.apple.quarantine` attribute, and Gatekeeper then blocks the unsigned
//! runtimes Lumina downloads (PHP, nginx, MySQL builds are not notarized). The
//! packages are verified by SHA-256 before they are extracted, so the
//! attribute is removed from the installed folder. Elsewhere this does nothing.

use std::path::Path;

#[cfg(target_os = "macos")]
pub fn clear(dir: &Path) {
    let status = std::process::Command::new("/usr/bin/xattr")
        .args(["-d", "-r", "com.apple.quarantine"])
        .arg(dir)
        .status();
    match status {
        // xattr exits 1 when no file had the attribute.
        Ok(s) if s.success() || s.code() == Some(1) => {}
        Ok(s) => tracing::warn!("xattr on {} exited with {s}", dir.display()),
        Err(err) => tracing::warn!("could not run xattr: {err}"),
    }
}

#[cfg(not(target_os = "macos"))]
pub fn clear(_dir: &Path) {}
