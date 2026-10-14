//! Runtime directory layout under the user-chosen data root.
//! See docs/architecture/runtime-layout.md.
//!
//! The data root is deliberately separate from the install directory so that app
//! updates and uninstalls never touch user data.

use std::path::{Path, PathBuf};

use crate::{Error, Result};

/// Top-level directories created inside the data root.
const DIRS: [&str; 7] = ["bin", "etc", "data", "ssl", "logs", "tmp", "www"];

/// Paths inside a Lumina data root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    root: PathBuf,
}

impl Layout {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `config.toml`, the user configuration.
    pub fn config_file(&self) -> PathBuf {
        self.root.join("config.toml")
    }

    /// Installed runtimes: `bin/<tool>/<version>/`.
    pub fn bin(&self) -> PathBuf {
        self.root.join("bin")
    }

    /// Rendered service configs.
    pub fn etc(&self) -> PathBuf {
        self.root.join("etc")
    }

    /// Database data directories.
    pub fn data(&self) -> PathBuf {
        self.root.join("data")
    }

    /// Local CA and site certificates.
    pub fn ssl(&self) -> PathBuf {
        self.root.join("ssl")
    }

    /// Service logs.
    pub fn logs(&self) -> PathBuf {
        self.root.join("logs")
    }

    /// Downloads, sessions, PID ledger.
    pub fn tmp(&self) -> PathBuf {
        self.root.join("tmp")
    }

    /// User projects; each folder becomes `<folder>.<tld>`.
    pub fn www(&self) -> PathBuf {
        self.root.join("www")
    }

    /// Creates the data root and every top-level directory. Idempotent.
    pub fn ensure(&self) -> Result<()> {
        for dir in DIRS {
            let path = self.root.join(dir);
            std::fs::create_dir_all(&path).map_err(|source| Error::Io { path, source })?;
        }
        Ok(())
    }
}

/// Suggested data root for the first-run wizard:
/// `C:\Lumina` on Windows (system drive), `~/Lumina` elsewhere.
pub fn default_data_root() -> PathBuf {
    if cfg!(windows) {
        let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_owned());
        return PathBuf::from(format!("{drive}\\Lumina"));
    }
    directories::UserDirs::new()
        .map(|dirs| dirs.home_dir().join("Lumina"))
        .unwrap_or_else(|| PathBuf::from("Lumina"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_creates_all_dirs_and_is_idempotent() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let layout = Layout::new(tmp.path().join("Lumina"));

        layout.ensure().expect("first ensure");
        layout.ensure().expect("second ensure");

        for dir in [
            layout.bin(),
            layout.etc(),
            layout.data(),
            layout.ssl(),
            layout.logs(),
            layout.tmp(),
            layout.www(),
        ] {
            assert!(dir.is_dir(), "{} missing", dir.display());
        }
    }

    #[test]
    fn ensure_reports_path_on_failure() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let file = tmp.path().join("not-a-dir");
        std::fs::write(&file, b"x").expect("write");

        let err = Layout::new(&file).ensure().expect_err("root is a file");
        assert!(matches!(err, Error::Io { .. }));
    }

    #[test]
    fn default_root_is_absolute_and_named_lumina() {
        let root = default_data_root();
        assert!(root.ends_with("Lumina"));
    }
}
