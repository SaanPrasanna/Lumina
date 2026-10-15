//! The bootstrap file: a tiny `bootstrap.toml` in the OS app-config directory that
//! records where the user put the data root. It is the only Lumina file outside the
//! data root, so the data root can live anywhere (another drive, a synced folder)
//! and survive reinstalls. Its absence means the first-run wizard has not completed.

use std::path::{Path, PathBuf};

use lumina_core::constants::{DEFAULT_LANGUAGE, SUPPORTED_LANGUAGES};
use lumina_core::{Error, Result};
use serde::{Deserialize, Serialize};

pub const FILE_NAME: &str = "bootstrap.toml";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct Bootstrap {
    /// Absolute path of the data root (`bin/`, `etc/`, `www/`, ...).
    pub data_root: PathBuf,
    /// UI language chosen in the first-run wizard.
    #[serde(default = "default_language")]
    pub language: String,
}

fn default_language() -> String {
    DEFAULT_LANGUAGE.to_owned()
}

impl Bootstrap {
    /// Validates user input from the first-run wizard.
    pub fn new(data_root: PathBuf, language: String) -> Result<Self> {
        if !data_root.is_absolute() {
            return Err(Error::Config(format!(
                "data root must be an absolute path: {}",
                data_root.display()
            )));
        }
        if !SUPPORTED_LANGUAGES.contains(&language.as_str()) {
            return Err(Error::Config(format!("unsupported language: {language}")));
        }
        Ok(Self {
            data_root,
            language,
        })
    }
}

/// Reads the bootstrap file. `Ok(None)` means first run.
pub fn load(path: &Path) -> Result<Option<Bootstrap>> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(Error::Io {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    let bootstrap =
        toml::from_str(&text).map_err(|e| Error::Config(format!("{}: {e}", path.display())))?;
    Ok(Some(bootstrap))
}

/// Writes the bootstrap file atomically.
pub fn save(path: &Path, bootstrap: &Bootstrap) -> Result<()> {
    let text = toml::to_string_pretty(bootstrap).map_err(|e| Error::Config(e.to_string()))?;
    lumina_core::fs::write_atomic(path, text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn abs_root() -> PathBuf {
        std::env::temp_dir().join("Lumina")
    }

    #[test]
    fn missing_file_means_first_run() {
        let tmp = tempfile::tempdir().expect("tempdir");
        assert_eq!(load(&tmp.path().join(FILE_NAME)).expect("load"), None);
    }

    #[test]
    fn round_trips() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join(FILE_NAME);
        let bootstrap = Bootstrap::new(abs_root(), "si".into()).expect("valid");

        save(&path, &bootstrap).expect("save");
        assert_eq!(load(&path).expect("load"), Some(bootstrap));
    }

    #[test]
    fn language_defaults_to_english_when_missing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join(FILE_NAME);
        let root = abs_root().display().to_string().replace('\\', "\\\\");
        std::fs::write(&path, format!("data_root = \"{root}\"\n")).expect("write");

        assert_eq!(load(&path).expect("load").expect("some").language, "en");
    }

    #[test]
    fn corrupt_file_is_a_config_error() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join(FILE_NAME);
        std::fs::write(&path, "not = [valid").expect("write");

        assert!(matches!(load(&path), Err(Error::Config(_))));
    }

    #[test]
    fn rejects_relative_root_and_unknown_language() {
        assert!(Bootstrap::new("Lumina".into(), "en".into()).is_err());
        assert!(Bootstrap::new(abs_root(), "xx".into()).is_err());
    }
}
