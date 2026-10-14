//! Install/remove the Lumina local root CA in the OS trust store.
//!
//! | OS | Store | Privileges |
//! |---|---|---|
//! | Windows | CurrentUser\Root | none; Windows shows its own confirmation |
//! | macOS | login keychain, trusted as root | the user's password prompt |
//! | Linux | system CA bundle (via `lumina-helper ca-install`) + the user's NSS databases (Chrome, Firefox) | pkexec for the system part |
//!
//! Firefox keeps its own store: on Windows and macOS Lumina turns on
//! `security.enterprise_roots.enabled` in each Firefox profile so it uses the
//! OS store; on Linux the CA is added to each profile's NSS database.
//!
//! All functions block (prompts wait for the user); call them from a blocking
//! thread.

use std::fmt;
use std::path::{Path, PathBuf};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

mod firefox;

/// Nickname used in NSS databases and the file name of the system copy.
pub const CA_NICKNAME: &str = "Lumina Local CA";

/// The CA certificate to trust or remove.
#[derive(Debug, Clone, Copy)]
pub struct CaCert<'a> {
    pub der: &'a [u8],
    /// The PEM file on disk (`<data-root>/ssl/ca.crt`).
    pub pem_path: &'a Path,
    /// SHA-1 of the DER, the "thumbprint" OS tools identify certificates by.
    pub sha1: [u8; 20],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustError {
    /// The user dismissed the OS confirmation or password prompt.
    Declined,
    /// The OS lacks the tools needed (e.g. no `update-ca-certificates`).
    Unavailable(String),
    Failed(String),
}

impl fmt::Display for TrustError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Declined => f.write_str("the confirmation was declined"),
            Self::Unavailable(why) | Self::Failed(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for TrustError {}

/// Whether the OS trusts `ca` as a root (no prompt).
pub fn is_trusted(ca: &CaCert<'_>) -> bool {
    #[cfg(target_os = "windows")]
    return windows::is_trusted(ca);
    #[cfg(target_os = "macos")]
    return macos::is_trusted(ca);
    #[cfg(target_os = "linux")]
    return linux::is_trusted(ca);
    #[allow(unreachable_code)]
    false
}

/// Adds `ca` to the OS trust store (one prompt) and makes Firefox use it.
/// `helper` is `lumina-helper`, needed on Linux for the system store.
pub fn install(ca: &CaCert<'_>, helper: &Path) -> Result<(), TrustError> {
    #[cfg(target_os = "windows")]
    windows::install(ca)?;
    #[cfg(target_os = "macos")]
    macos::install(ca)?;
    #[cfg(target_os = "linux")]
    linux::install(ca, helper)?;
    let _ = helper;
    firefox::after_install(ca);
    Ok(())
}

/// Removes `ca` from the OS trust store (one prompt).
pub fn remove(ca: &CaCert<'_>, helper: &Path) -> Result<(), TrustError> {
    firefox::before_remove(ca);
    let _ = helper;
    #[cfg(target_os = "windows")]
    return windows::remove(ca);
    #[cfg(target_os = "macos")]
    return macos::remove(ca);
    #[cfg(target_os = "linux")]
    return linux::remove(ca, helper);
    #[allow(unreachable_code)]
    Err(TrustError::Unavailable("unsupported platform".into()))
}

/// Linux system-store paths: (directory, update command). Debian/Ubuntu first,
/// then Fedora/RHEL/Arch.
pub const LINUX_SYSTEM_STORES: [(&str, &str); 2] = [
    ("/usr/local/share/ca-certificates", "update-ca-certificates"),
    ("/etc/pki/ca-trust/source/anchors", "update-ca-trust"),
];

/// File name of the CA in the Linux system store.
pub fn linux_system_file_name() -> String {
    "lumina-local-ca.crt".to_owned()
}

/// Writes `pem` into the first Linux system store that exists and refreshes
/// the bundle. Runs as root inside `lumina-helper`.
pub fn linux_system_install(pem: &[u8]) -> Result<PathBuf, String> {
    for (dir, update) in LINUX_SYSTEM_STORES {
        let dir = Path::new(dir);
        if !dir.is_dir() {
            continue;
        }
        let target = dir.join(linux_system_file_name());
        std::fs::write(&target, pem).map_err(|e| format!("{}: {e}", target.display()))?;
        run_update(update)?;
        return Ok(target);
    }
    Err("no system CA folder found (/usr/local/share/ca-certificates or /etc/pki/ca-trust/source/anchors)".into())
}

/// Removes Lumina's CA file from every Linux system store and refreshes them.
/// Runs as root inside `lumina-helper`.
pub fn linux_system_remove() -> Result<(), String> {
    for (dir, update) in LINUX_SYSTEM_STORES {
        let target = Path::new(dir).join(linux_system_file_name());
        if target.exists() {
            std::fs::remove_file(&target).map_err(|e| format!("{}: {e}", target.display()))?;
            run_update(update)?;
        }
    }
    Ok(())
}

fn run_update(program: &str) -> Result<(), String> {
    let mut cmd = std::process::Command::new(program);
    if program == "update-ca-certificates" {
        cmd.arg("--fresh");
    }
    let out = cmd.output().map_err(|e| format!("{program}: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{program} failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// Hex thumbprint, as OS tools print it.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}
