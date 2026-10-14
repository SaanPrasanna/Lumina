//! Firefox keeps its own certificate store. On Windows and macOS it can use
//! the OS store instead when `security.enterprise_roots.enabled` is on; Lumina
//! turns that on in each profile's `user.js` after trusting the CA. (On Linux
//! the CA goes into each profile's NSS database instead, see `linux.rs`.)

use std::path::{Path, PathBuf};

use super::CaCert;

const PREF: &str = r#"user_pref("security.enterprise_roots.enabled", true);"#;
const MARKER: &str =
    "// Added by Lumina: trust the certificates installed in the operating system.";

pub(super) fn after_install(_ca: &CaCert<'_>) {
    if cfg!(target_os = "linux") {
        return;
    }
    for profile in profiles() {
        if let Err(err) = enable_os_roots(&profile) {
            tracing::warn!("Firefox profile {}: {err}", profile.display());
        }
    }
}

/// The pref is left in place on removal: it only tells Firefox to follow
/// the OS store, which no longer contains the CA.
pub(super) fn before_remove(_ca: &CaCert<'_>) {}

fn profiles_root() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("Mozilla/Firefox/Profiles"))
    } else {
        std::env::var_os("HOME")
            .map(|h| PathBuf::from(h).join("Library/Application Support/Firefox/Profiles"))
    }
}

fn profiles() -> Vec<PathBuf> {
    let Some(root) = profiles_root() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("prefs.js").is_file())
        .collect()
}

/// Appends the pref to `user.js` unless it is already there.
pub(super) fn enable_os_roots(profile: &Path) -> std::io::Result<bool> {
    let user_js = profile.join("user.js");
    let current = std::fs::read_to_string(&user_js).unwrap_or_default();
    if current.contains("security.enterprise_roots.enabled") {
        return Ok(false);
    }
    let mut next = current;
    if !next.is_empty() && !next.ends_with('\n') {
        next.push('\n');
    }
    next.push_str(MARKER);
    next.push('\n');
    next.push_str(PREF);
    next.push('\n');
    std::fs::write(&user_js, next)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pref_is_added_once_and_existing_settings_are_respected() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join("user.js"), "user_pref(\"a\", 1);").expect("write");
        assert!(enable_os_roots(tmp.path()).expect("first"));
        assert!(!enable_os_roots(tmp.path()).expect("second"));
        let text = std::fs::read_to_string(tmp.path().join("user.js")).expect("read");
        assert!(text.starts_with("user_pref(\"a\", 1);\n// Added by Lumina"));
        assert_eq!(text.matches("enterprise_roots").count(), 1);

        // A user who set it to false keeps their choice.
        let other = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            other.path().join("user.js"),
            "user_pref(\"security.enterprise_roots.enabled\", false);\n",
        )
        .expect("write");
        assert!(!enable_os_roots(other.path()).expect("respect"));
    }
}
