//! trust_store: linux implementation. The system bundle needs root, so
//! `lumina-helper ca-install` writes it through pkexec. Chrome and Firefox
//! read NSS databases instead, which belong to the user and are updated with
//! `certutil` (package `libnss3-tools` / `nss-tools`) when it is installed.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::{CA_NICKNAME, CaCert, LINUX_SYSTEM_STORES, TrustError, linux_system_file_name};
use crate::elevation::{self, ElevationError};

pub(super) fn is_trusted(ca: &CaCert<'_>) -> bool {
    let Ok(ours) = std::fs::read(ca.pem_path) else {
        return false;
    };
    LINUX_SYSTEM_STORES.iter().any(|(dir, _)| {
        std::fs::read(Path::new(dir).join(linux_system_file_name())).is_ok_and(|c| c == ours)
    })
}

fn helper(helper: &Path, args: &[&std::ffi::OsStr]) -> Result<(), TrustError> {
    match elevation::run_elevated(helper, args) {
        Ok(exit) if exit.code == 0 => Ok(()),
        Ok(exit) => Err(TrustError::Failed(format!(
            "lumina-helper failed (exit code {}): {}",
            exit.code, exit.stderr
        ))),
        Err(ElevationError::Declined) => Err(TrustError::Declined),
        Err(e) => Err(TrustError::Unavailable(e.to_string())),
    }
}

pub(super) fn install(ca: &CaCert<'_>, helper_path: &Path) -> Result<(), TrustError> {
    helper(
        helper_path,
        &["ca-install".as_ref(), ca.pem_path.as_os_str()],
    )?;
    for db in nss_databases() {
        let _ = certutil(
            &db,
            &["-A", "-t", "C,,", "-n", CA_NICKNAME, "-i"],
            Some(ca.pem_path),
        );
    }
    Ok(())
}

pub(super) fn remove(_ca: &CaCert<'_>, helper_path: &Path) -> Result<(), TrustError> {
    helper(helper_path, &["ca-remove".as_ref()])?;
    for db in nss_databases() {
        let _ = certutil(&db, &["-D", "-n", CA_NICKNAME], None);
    }
    Ok(())
}

/// The user's NSS databases: Chrome/Chromium (`~/.pki/nssdb`) and every
/// Firefox profile (regular and snap).
fn nss_databases() -> Vec<PathBuf> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return Vec::new();
    };
    let mut dbs = vec![home.join(".pki/nssdb")];
    for root in [".mozilla/firefox", "snap/firefox/common/.mozilla/firefox"] {
        if let Ok(entries) = std::fs::read_dir(home.join(root)) {
            dbs.extend(
                entries
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| p.join("cert9.db").is_file()),
            );
        }
    }
    dbs.retain(|d| d.join("cert9.db").is_file());
    dbs
}

fn certutil(db: &Path, args: &[&str], file: Option<&Path>) -> std::io::Result<bool> {
    let mut cmd = Command::new("certutil");
    cmd.arg("-d")
        .arg(format!("sql:{}", db.display()))
        .args(args);
    if let Some(file) = file {
        cmd.arg(file);
    }
    Ok(cmd.output()?.status.success())
}
