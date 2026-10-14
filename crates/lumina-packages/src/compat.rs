//! Compatibility rules, e.g. phpMyAdmin 6 requires PHP >= 8.4, otherwise phpMyAdmin 5.
//!
//! Rules are data: each release carries `requires.php` (a semver
//! requirement), and Lumina picks the newest installed release whose
//! requirement the active PHP meets.

use semver::{Version, VersionReq};

use crate::installed::{Installed, version_key};

/// Parses "8.4", "8.4.26" or "8.4.0RC1" into a semver version.
fn php_version(v: &str) -> Option<Version> {
    let nums: Vec<u64> = version_key(v).into_iter().take(3).collect();
    let get = |i: usize| nums.get(i).copied().unwrap_or(0);
    (!nums.is_empty()).then(|| Version::new(get(0), get(1), get(2)))
}

/// Whether PHP `php` satisfies `requirement` (an unparsable requirement is
/// treated as satisfied, so a typo never hides a working install).
pub fn php_satisfies(php: &str, requirement: Option<&str>) -> bool {
    let Some(req) = requirement.map(str::trim).filter(|r| !r.is_empty()) else {
        return true;
    };
    let (Some(version), Ok(req)) = (php_version(php), VersionReq::parse(req)) else {
        return true;
    };
    req.matches(&version)
}

/// The newest of `installs` (newest first) that runs on PHP `php`. Versions
/// installed by hand have no metadata and count as compatible.
pub fn newest_for_php<'a>(installs: &'a [Installed], php: &str) -> Option<&'a Installed> {
    installs
        .iter()
        .find(|i| php_satisfies(php, i.meta.as_ref().and_then(|m| m.requires.php.as_deref())))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::catalog::Requires;
    use crate::installed::Meta;

    fn pma(version: &str, php: &str) -> Installed {
        Installed {
            version: version.into(),
            dir: PathBuf::from(version),
            meta: Some(Meta {
                package: "phpmyadmin".into(),
                version: version.into(),
                url: String::new(),
                sha256: String::new(),
                installed_at: 0,
                requires: Requires {
                    php: Some(php.into()),
                },
            }),
        }
    }

    #[test]
    fn requirement_matching() {
        assert!(php_satisfies("8.4.26", Some(">=8.4")));
        assert!(!php_satisfies("8.3.35", Some(">=8.4")));
        assert!(php_satisfies("8.3.35", Some(">=7.2.5, <8.4")));
        assert!(!php_satisfies("8.4.0", Some(">=7.2.5, <8.4")));
        assert!(php_satisfies("8.4", None));
        assert!(php_satisfies("8.4", Some("not a requirement")));
    }

    /// The blueprint's rule: phpMyAdmin 6 on PHP 8.4+, phpMyAdmin 5 below.
    #[test]
    fn picks_the_newest_compatible_db_ui() {
        let installs = vec![pma("6.0.0", ">=8.4"), pma("5.2.3", ">=7.2.5")];
        assert_eq!(
            newest_for_php(&installs, "8.4.26").map(|i| i.version.as_str()),
            Some("6.0.0")
        );
        assert_eq!(
            newest_for_php(&installs, "8.3.35").map(|i| i.version.as_str()),
            Some("5.2.3")
        );
        assert!(newest_for_php(&installs[..1], "8.2.1").is_none());
    }
}
