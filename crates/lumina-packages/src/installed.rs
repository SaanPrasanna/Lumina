//! Index of installed versions on disk.
//!
//! `bin/<tool>/<version>/` is the source of truth: a folder there is an
//! installed version, whether Quick Add put it there or the user unpacked it by
//! hand. Quick Add also writes `.lumina-package.json` into the folder, so
//! Lumina knows where it came from and what it requires.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::catalog::{Release, Requires};
use crate::{Error, Result};

pub const META_FILE: &str = ".lumina-package.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    pub package: String,
    pub version: String,
    pub url: String,
    pub sha256: String,
    /// Unix seconds.
    pub installed_at: i64,
    #[serde(default)]
    pub requires: Requires,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Installed {
    pub version: String,
    pub dir: PathBuf,
    /// `None` for versions installed by hand.
    pub meta: Option<Meta>,
}

/// Versions under `root` (`bin/<tool>`), newest first. Unfinished
/// `.partial` folders are ignored.
pub fn list(root: &Path) -> Vec<Installed> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut out: Vec<Installed> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let version = e.file_name().to_str()?.to_owned();
            if version.ends_with(".partial") || version.starts_with('.') {
                return None;
            }
            let dir = e.path();
            let meta = std::fs::read_to_string(dir.join(META_FILE))
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok());
            Some(Installed { version, dir, meta })
        })
        .collect();
    out.sort_by_key(|i| std::cmp::Reverse(version_key(&i.version)));
    out
}

pub fn write_meta(dir: &Path, package: &str, release: &Release) -> Result<()> {
    let meta = Meta {
        package: package.to_owned(),
        version: release.version.clone(),
        url: release.url.clone(),
        sha256: release.sha256.clone(),
        installed_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or_default(),
        requires: release.requires.clone(),
    };
    let path = dir.join(META_FILE);
    let json = serde_json::to_vec_pretty(&meta).map_err(|e| Error::Catalog(e.to_string()))?;
    std::fs::write(&path, json).map_err(|source| Error::Io { path, source })
}

/// Numeric sort key: "8.10.2" > "8.9.0".
pub fn version_key(v: &str) -> Vec<u64> {
    v.split(|c: char| !c.is_ascii_digit())
        .filter(|p| !p.is_empty())
        .map(|p| p.parse().unwrap_or(0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_folders_newest_first_with_metadata() {
        let tmp = tempfile::tempdir().expect("tempdir");
        for v in ["8.4.26", "8.10.1", "8.3.35", "9.0.0.partial"] {
            std::fs::create_dir_all(tmp.path().join(v)).expect("mkdir");
        }
        std::fs::write(tmp.path().join("notes.txt"), "x").expect("file");
        std::fs::write(
            tmp.path().join("8.4.26").join(META_FILE),
            r#"{"package":"php","version":"8.4.26","url":"https://x","sha256":"ab","installedAt":1,"requires":{}}"#,
        )
        .expect("meta");
        let list = list(tmp.path());
        let versions: Vec<&str> = list.iter().map(|i| i.version.as_str()).collect();
        assert_eq!(versions, ["8.10.1", "8.4.26", "8.3.35"]);
        assert!(list[1].meta.is_some() && list[0].meta.is_none());
        assert!(super::list(&tmp.path().join("missing")).is_empty());
    }
}
