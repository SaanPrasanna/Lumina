//! Installed tool versions under `<data-root>/bin/<tool>/<version>/`, shared by
//! PHP, nginx and Apache.

use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::Duration;

use lumina_core::{Error, Result};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Install {
    /// As reported by the tool, e.g. `1.30.5`.
    pub version: String,
    /// The install root (the `<version>` folder).
    pub dir: PathBuf,
    /// The main executable.
    pub exe: PathBuf,
}

/// Every install under `root` that has one of `exe_candidates` (paths relative
/// to the install folder, without `.exe`). The version comes from running the
/// executable with `version_args` and `parse` on its stdout + stderr, or the
/// folder name if that fails. Newest first.
pub async fn discover(
    root: &Path,
    exe_candidates: &[&str],
    version_args: &[&str],
    parse: fn(&str) -> Option<String>,
) -> Vec<Install> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut installs = Vec::new();
    for entry in entries.flatten() {
        let dir = entry.path();
        let Some(exe) = exe_candidates
            .iter()
            .map(|rel| dir.join(format!("{rel}{}", std::env::consts::EXE_SUFFIX)))
            .find(|p| p.is_file())
        else {
            continue;
        };
        let reported = match cached_version(&exe) {
            Some(cached) => cached,
            None => {
                let version = run(&exe, version_args, Duration::from_secs(10))
                    .await
                    .ok()
                    .and_then(|out| {
                        let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
                        text.push_str(&String::from_utf8_lossy(&out.stderr));
                        parse(&text)
                    });
                remember_version(&exe, version.clone());
                version
            }
        };
        let version = reported.unwrap_or_else(|| entry.file_name().to_string_lossy().into_owned());
        installs.push(Install { version, dir, exe });
    }
    installs.sort_by_key(|i| std::cmp::Reverse(version_key(&i.version)));
    installs
}

/// What identifies an executable's build: path, size and modification time.
type ExeKey = (PathBuf, u64, Option<std::time::SystemTime>);

/// Versions reported by executables, so listing installs (which the UI does
/// often) does not start `httpd -v`, `php-cgi -v`, ... every time. A replaced
/// executable has another size or time and is asked again.
static VERSIONS: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<ExeKey, Option<String>>>,
> = std::sync::LazyLock::new(Default::default);

fn exe_key(exe: &Path) -> Option<ExeKey> {
    let meta = std::fs::metadata(exe).ok()?;
    Some((exe.to_path_buf(), meta.len(), meta.modified().ok()))
}

fn cached_version(exe: &Path) -> Option<Option<String>> {
    let key = exe_key(exe)?;
    VERSIONS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&key)
        .cloned()
}

fn remember_version(exe: &Path, version: Option<String>) {
    if let Some(key) = exe_key(exe) {
        VERSIONS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key, version);
    }
}

/// Picks an install: empty → newest; `"8.4"` → newest 8.4.x; `"8.4.26"` → exact.
pub fn select<'a>(installs: &'a [Install], wanted: &str) -> Option<&'a Install> {
    let wanted = wanted.trim();
    if wanted.is_empty() {
        return installs.first();
    }
    installs
        .iter()
        .find(|i| i.version == wanted || i.version.starts_with(&format!("{wanted}.")))
}

/// `Error::Service` explaining that `wanted` (or anything) is not installed.
pub fn not_installed(service: &str, tool: &str, wanted: &str, root: &Path) -> Error {
    let what = if wanted.trim().is_empty() {
        format!("{tool} is not installed")
    } else {
        format!("{tool} {wanted} is not installed")
    };
    Error::Service(service.into(), format!("{what} in {}", root.display()))
}

/// Runs a short-lived helper command (version query, config test, signal)
/// without a console window, with a time limit.
pub async fn run(exe: &Path, args: &[&str], limit: Duration) -> Result<Output> {
    let mut cmd = std::process::Command::new(exe);
    cmd.args(args);
    if let Some(dir) = exe.parent() {
        cmd.current_dir(dir);
    }
    lumina_platform::command::hide_console(&mut cmd);
    let io = |source| Error::Io {
        path: exe.to_path_buf(),
        source,
    };
    let mut cmd = tokio::process::Command::from(cmd);
    cmd.kill_on_drop(true);
    tokio::time::timeout(limit, cmd.output())
        .await
        .map_err(|_| Error::Other(format!("{} {} timed out", exe.display(), args.join(" "))))?
        .map_err(io)
}

/// Numeric sort key: "8.10.1" > "8.9.30"; non-numeric suffixes are ignored.
pub fn version_key(version: &str) -> Vec<u32> {
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

    fn install(v: &str) -> Install {
        Install {
            version: v.into(),
            dir: PathBuf::from(v),
            exe: PathBuf::from(v),
        }
    }

    #[test]
    fn version_cache_is_keyed_on_the_file() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let exe = tmp.path().join("httpd.exe");
        std::fs::write(&exe, b"build 1").expect("write");
        assert_eq!(cached_version(&exe), None, "not asked yet");
        remember_version(&exe, Some("2.4.69".into()));
        assert_eq!(cached_version(&exe), Some(Some("2.4.69".into())));

        // A replaced binary (other size) is asked again.
        std::fs::write(&exe, b"build two").expect("rewrite");
        assert_eq!(cached_version(&exe), None);
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
