//! Discovers installed PHP versions under `<data-root>/bin/php/<version>/`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhpInstall {
    /// Reported by `php-cgi -v`, e.g. `8.4.26`.
    pub version: String,
    /// The PHP root folder (contains `ext/`).
    pub dir: PathBuf,
    pub cgi: PathBuf,
}

impl PhpInstall {
    pub fn ext_dir(&self) -> PathBuf {
        self.dir.join("ext")
    }
}

/// `php-cgi.exe` on Windows, `php-cgi` elsewhere.
pub fn cgi_file_name() -> String {
    format!("php-cgi{}", std::env::consts::EXE_SUFFIX)
}

/// All usable PHP installs, newest first.
pub async fn discover(php_root: &Path) -> Vec<PhpInstall> {
    let Ok(entries) = std::fs::read_dir(php_root) else {
        return Vec::new();
    };
    let mut installs = Vec::new();
    for entry in entries.flatten() {
        let dir = entry.path();
        // Windows zips put php-cgi at the root; Unix builds use bin/.
        let Some(cgi) = [
            dir.join(cgi_file_name()),
            dir.join("bin").join(cgi_file_name()),
        ]
        .into_iter()
        .find(|p| p.is_file()) else {
            continue;
        };
        let fallback = entry.file_name().to_string_lossy().into_owned();
        let version = detect_version(&cgi).await.unwrap_or(fallback);
        installs.push(PhpInstall { version, dir, cgi });
    }
    installs.sort_by_key(|i| std::cmp::Reverse(version_key(&i.version)));
    installs
}

/// Picks an install: empty → newest; `"8.4"` → newest 8.4.x; `"8.4.26"` → exact.
pub fn select<'a>(installs: &'a [PhpInstall], wanted: &str) -> Option<&'a PhpInstall> {
    let wanted = wanted.trim();
    if wanted.is_empty() {
        return installs.first();
    }
    installs
        .iter()
        .find(|i| i.version == wanted || i.version.starts_with(&format!("{wanted}.")))
}

/// Runs `php-cgi -v` and extracts the version from "PHP 8.4.26 (cgi-fcgi) ...".
async fn detect_version(cgi: &Path) -> Option<String> {
    let mut cmd = std::process::Command::new(cgi);
    cmd.arg("-v");
    lumina_platform::command::hide_console(&mut cmd);
    let output = tokio::time::timeout(
        Duration::from_secs(10),
        tokio::process::Command::from(cmd).output(),
    )
    .await
    .ok()?
    .ok()?;
    parse_version(&String::from_utf8_lossy(&output.stdout))
}

fn parse_version(text: &str) -> Option<String> {
    let rest = text.trim_start().strip_prefix("PHP ")?;
    let version = rest.split_whitespace().next()?;
    version
        .starts_with(|c: char| c.is_ascii_digit())
        .then(|| version.to_owned())
}

/// Numeric sort key: "8.10.1" > "8.9.30"; non-numeric suffixes are ignored.
fn version_key(version: &str) -> Vec<u32> {
    version
        .split('.')
        .map(|part| {
            part.chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse()
                .unwrap_or(0)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn install(v: &str) -> PhpInstall {
        PhpInstall {
            version: v.into(),
            dir: PathBuf::from(v),
            cgi: PathBuf::from(v),
        }
    }

    #[test]
    fn parses_php_cgi_banner() {
        let out = "PHP 8.4.26 (cgi-fcgi) (built: Sep 22 2026 15:11:32) (NTS Visual C++ 2022 x64)\n";
        assert_eq!(parse_version(out).as_deref(), Some("8.4.26"));
        assert_eq!(parse_version("garbage"), None);
    }

    #[test]
    fn sorts_numerically() {
        let mut v = ["8.9.30", "8.10.1", "8.4.26"].map(String::from).to_vec();
        v.sort_by_key(|s| std::cmp::Reverse(version_key(s)));
        assert_eq!(v, ["8.10.1", "8.9.30", "8.4.26"]);
    }

    #[test]
    fn selects_newest_prefix_or_exact() {
        let installs = vec![install("8.4.26"), install("8.3.35"), install("8.3.10")];
        assert_eq!(select(&installs, "").map(|i| &*i.version), Some("8.4.26"));
        assert_eq!(
            select(&installs, "8.3").map(|i| &*i.version),
            Some("8.3.35")
        );
        assert_eq!(
            select(&installs, "8.3.10").map(|i| &*i.version),
            Some("8.3.10")
        );
        assert_eq!(select(&installs, "8.2"), None);
        // "8.3" must not match "8.30.x".
        assert_eq!(select(&[install("8.30.1")], "8.3"), None);
    }
}
