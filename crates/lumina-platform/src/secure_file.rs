//! Private keys readable by the current user only.
//!
//! On Windows a folder like `C:\Lumina` inherits an ACL that lets every
//! signed-in user modify it, so key files get their inheritance removed and a
//! single "full control" entry for the current user (plus SYSTEM and
//! Administrators, which can read everything anyway). On Unix: mode 0600.

use std::path::Path;

/// Writes `contents` to `path` and restricts it to the current user.
pub fn write_private(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Restrict an empty file before the secret is in it.
    std::fs::write(path, b"")?;
    restrict_to_owner(path)?;
    std::fs::write(path, contents)
}

#[cfg(unix)]
pub fn restrict_to_owner(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
}

#[cfg(windows)]
pub fn restrict_to_owner(path: &Path) -> std::io::Result<()> {
    use crate::command::hide_console;
    let user = match (std::env::var("USERDOMAIN"), std::env::var("USERNAME")) {
        (Ok(domain), Ok(name)) => format!("{domain}\\{name}"),
        (_, Ok(name)) => name,
        _ => return Err(std::io::Error::other("unknown user")),
    };
    let mut cmd = std::process::Command::new("icacls");
    cmd.arg(path).args([
        "/inheritance:r",
        "/grant:r",
        &format!("{user}:F"),
        // Well-known SIDs, so the names need no translation on localised Windows.
        "/grant:r",
        "*S-1-5-18:F",
        "/grant:r",
        "*S-1-5-32-544:F",
    ]);
    hide_console(&mut cmd);
    let out = cmd.output()?;
    if out.status.success() {
        return Ok(());
    }
    Err(std::io::Error::other(format!(
        "icacls failed: {}",
        String::from_utf8_lossy(&out.stdout).trim()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_file_is_written_and_restricted() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let key = tmp.path().join("ssl").join("ca.key");
        write_private(&key, b"secret").expect("write");
        assert_eq!(std::fs::read(&key).expect("read"), b"secret");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&key).expect("meta").permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        #[cfg(windows)]
        {
            let out = std::process::Command::new("icacls")
                .arg(&key)
                .output()
                .expect("icacls");
            let acl = String::from_utf8_lossy(&out.stdout);
            assert!(!acl.contains("(I)"), "inherited entries left: {acl}");
            assert!(!acl.contains("Users:"), "{acl}");
        }
        // Can be rewritten by its owner.
        write_private(&key, b"again").expect("rewrite");
    }
}
