//! trust_store: macos implementation (`security`, the login keychain, trusted
//! as a root for the user; macOS asks for the user's password).

use std::path::PathBuf;
use std::process::Command;

use super::{CaCert, TrustError, hex};

fn login_keychain() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    home.join("Library/Keychains/login.keychain-db")
}

pub(super) fn is_trusted(ca: &CaCert<'_>) -> bool {
    // Evaluates the certificate against the user's and the system's trust
    // settings; exits 0 only when it is a trusted root.
    Command::new("/usr/bin/security")
        .args(["verify-cert", "-L", "-p", "ssl", "-c"])
        .arg(ca.pem_path)
        .output()
        .is_ok_and(|o| o.status.success())
}

fn run(args: &[&std::ffi::OsStr]) -> Result<(), TrustError> {
    let out = Command::new("/usr/bin/security")
        .args(args)
        .output()
        .map_err(|e| TrustError::Unavailable(format!("security: {e}")))?;
    if out.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_owned();
    // "The authorization was canceled by the user." / errAuthorizationCanceled
    if stderr.contains("canceled") || stderr.contains("-60006") || stderr.contains("-128") {
        return Err(TrustError::Declined);
    }
    Err(TrustError::Failed(stderr))
}

pub(super) fn install(ca: &CaCert<'_>) -> Result<(), TrustError> {
    let keychain = login_keychain();
    run(&[
        "add-trusted-cert".as_ref(),
        "-r".as_ref(),
        "trustRoot".as_ref(),
        "-k".as_ref(),
        keychain.as_os_str(),
        ca.pem_path.as_os_str(),
    ])
}

pub(super) fn remove(ca: &CaCert<'_>) -> Result<(), TrustError> {
    let sha1 = hex(&ca.sha1);
    // Drop the trust setting first (prompts), then the certificate itself.
    run(&["remove-trusted-cert".as_ref(), ca.pem_path.as_os_str()])?;
    let keychain = login_keychain();
    let _ = run(&[
        "delete-certificate".as_ref(),
        "-Z".as_ref(),
        sha1.as_ref(),
        keychain.as_os_str(),
    ]);
    Ok(())
}
