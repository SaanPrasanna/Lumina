//! Remote + bundled package catalog (resources/catalog/packages.json) per OS/arch.
//!
//! Every release names its download URL (plus mirrors), its SHA-256 and how
//! the archive is laid out. The bundled catalog ships with the app, so it is
//! as trustworthy as the app itself. A remote, signed catalog can be added
//! later without changing this format.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Catalog {
    pub schema_version: u32,
    #[serde(default)]
    pub updated: String,
    pub packages: Vec<Package>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Run by Lumina (PHP, web servers, databases, Redis).
    Service,
    /// Runtimes for the terminal (Node.js, Python).
    Runtime,
    /// Command-line tools (Composer).
    Tool,
    /// PHP applications Lumina serves (phpMyAdmin).
    Webapp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Package {
    /// Also the folder name under `bin/`.
    pub id: String,
    pub name: String,
    pub kind: Kind,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub homepage: String,
    pub versions: Vec<Release>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Archive {
    Zip,
    #[serde(rename = "tar.gz")]
    TarGz,
    /// A single file (e.g. `composer.phar`), saved as `file_name`.
    File,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Requires {
    /// Semver requirement on the active PHP, e.g. `">=8.4"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub php: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Release {
    pub version: String,
    /// `windows`, `macos`, `linux` or `any`.
    pub os: String,
    /// `x86_64`, `aarch64` or `any`.
    pub arch: String,
    pub url: String,
    /// Tried in order when `url` fails.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mirrors: Vec<String>,
    /// Lowercase hex.
    pub sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    pub archive: Archive,
    /// Leading path components to drop (`nginx-1.30.5/…` → 1).
    #[serde(default)]
    pub strip: u32,
    /// Only this folder inside the archive is installed (`Apache24`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subdir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    /// `LTS`, `mainline`, `end of life`, ...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    #[serde(default)]
    pub requires: Requires,
    /// Where the hash came from, for the record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checksum_source: Option<String>,
}

impl Release {
    /// Whether this release runs on `os`/`arch` (`any` matches everything).
    pub fn fits(&self, os: &str, arch: &str) -> bool {
        (self.os == "any" || self.os == os) && (self.arch == "any" || self.arch == arch)
    }

    pub fn urls(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.url.as_str()).chain(self.mirrors.iter().map(String::as_str))
    }
}

/// This machine, in catalog terms.
pub fn platform() -> (&'static str, &'static str) {
    (std::env::consts::OS, std::env::consts::ARCH)
}

impl Catalog {
    pub fn parse(json: &str) -> Result<Self> {
        let catalog: Catalog = serde_json::from_str(json)
            .map_err(|e| Error::Catalog(format!("unreadable catalog: {e}")))?;
        if catalog.schema_version != SCHEMA_VERSION {
            return Err(Error::Catalog(format!(
                "catalog schema {} is not supported (expected {SCHEMA_VERSION})",
                catalog.schema_version
            )));
        }
        for package in &catalog.packages {
            if !is_safe_id(&package.id) {
                return Err(Error::Catalog(format!("bad package id {:?}", package.id)));
            }
            for r in &package.versions {
                let hex = r.sha256.len() == 64 && r.sha256.bytes().all(|b| b.is_ascii_hexdigit());
                if !hex || !is_safe_id(&r.version) || !r.urls().all(|u| u.starts_with("https://")) {
                    return Err(Error::Catalog(format!(
                        "{} {}: needs an https URL, a SHA-256 and a plain version",
                        package.id, r.version
                    )));
                }
            }
        }
        Ok(catalog)
    }

    pub fn load(path: &Path) -> Result<Self> {
        let json = std::fs::read_to_string(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        Self::parse(&json)
    }

    pub fn package(&self, id: &str) -> Option<&Package> {
        self.packages.iter().find(|p| p.id == id)
    }

    /// The release of `id` `version` for this machine.
    pub fn release(&self, id: &str, version: &str) -> Option<&Release> {
        let (os, arch) = platform();
        self.package(id)?
            .versions
            .iter()
            .find(|r| r.version == version && r.fits(os, arch))
    }
}

/// Package ids and versions become folder names: letters, digits, `.`, `-`,
/// `_`, `+`, and never `..`.
pub fn is_safe_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && !s.contains("..")
        && !s.starts_with('.')
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'+'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundled() -> Catalog {
        Catalog::load(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/catalog/packages.json"),
        )
        .expect("bundled catalog")
    }

    #[test]
    fn bundled_catalog_is_valid() {
        let catalog = bundled();
        for id in [
            "php",
            "nginx",
            "apache",
            "mysql",
            "mariadb",
            "redis",
            "phpmyadmin",
            "node",
            "python",
            "composer",
        ] {
            assert!(catalog.package(id).is_some(), "{id} missing");
        }
        let pma = catalog.package("phpmyadmin").expect("pma");
        assert!(pma.versions.iter().all(|r| r.requires.php.is_some()));
        let composer = &catalog.package("composer").expect("composer").versions[0];
        assert_eq!(composer.archive, Archive::File);
        assert_eq!(composer.file_name.as_deref(), Some("composer.phar"));
    }

    #[test]
    fn platform_filter() {
        let catalog = bundled();
        let node = catalog.package("node").expect("node");
        let win: Vec<_> = node
            .versions
            .iter()
            .filter(|r| r.fits("windows", "x86_64"))
            .collect();
        assert!(win.iter().all(|r| r.archive == Archive::Zip));
        assert!(
            node.versions
                .iter()
                .any(|r| r.fits("macos", "aarch64") && r.archive == Archive::TarGz)
        );
        assert!(catalog.package("composer").expect("c").versions[0].fits("linux", "aarch64"));
    }

    #[test]
    fn rejects_unsafe_entries() {
        let base = r#"{"schema_version":1,"packages":[{"id":"x","name":"X","kind":"tool","versions":[{"version":"1.0","os":"any","arch":"any","url":"https://a/b","sha256":"0000000000000000000000000000000000000000000000000000000000000000","archive":"file"}]}]}"#;
        assert!(Catalog::parse(base).is_ok());
        for bad in [
            base.replace("https://a/b", "http://a/b"),
            base.replace("\"1.0\"", "\"../1.0\""),
            base.replace("\"id\":\"x\"", "\"id\":\"a/b\""),
            base.replace(
                "0000000000000000000000000000000000000000000000000000000000000000",
                "00",
            ),
            base.replace("\"schema_version\":1", "\"schema_version\":2"),
        ] {
            assert!(Catalog::parse(&bad).is_err(), "{bad}");
        }
    }
}
