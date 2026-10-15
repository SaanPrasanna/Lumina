//! Reading and writing `<data-root>/config.toml`.
//!
//! - Missing file: the commented default template is written.
//! - Corrupt file: it is kept as `config.toml.corrupt`, and `config.toml.bak` (the
//!   version before the last save) is restored; failing that, defaults are used.
//! - Saving: the current file becomes `config.toml.bak`, then the new content is
//!   written atomically.

use std::path::{Path, PathBuf};

use lumina_core::{Error, Result};
use serde::Serialize;

use crate::defaults::DEFAULT_TOML;
use crate::migrate::migrate;
use crate::schema::AppConfig;
use crate::validate::{Warning, validate};

/// Where the loaded configuration came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum ConfigSource {
    /// Read from config.toml.
    File,
    /// config.toml did not exist; defaults were written.
    Created,
    /// config.toml was unreadable; config.toml.bak was restored.
    RestoredFromBackup,
    /// config.toml and its backup were unreadable; defaults were written.
    ResetToDefaults,
}

#[derive(Debug, Clone)]
pub struct Loaded {
    pub config: AppConfig,
    pub warnings: Vec<Warning>,
    pub source: ConfigSource,
}

pub fn backup_path(path: &Path) -> PathBuf {
    with_suffix(path, ".bak")
}

pub fn corrupt_path(path: &Path) -> PathBuf {
    with_suffix(path, ".corrupt")
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

/// Loads `path`, creating or repairing it as described in the module docs.
/// Fails only on I/O errors or a config written by a newer Lumina (which must
/// not be overwritten).
pub fn load_or_create(path: &Path) -> Result<Loaded> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            lumina_core::fs::write_atomic(path, DEFAULT_TOML.as_bytes())?;
            return finish(parse(DEFAULT_TOML)?, ConfigSource::Created, Vec::new());
        }
        Err(source) => {
            return Err(Error::Io {
                path: path.to_path_buf(),
                source,
            });
        }
    };

    let parse_error = match parse(&text) {
        Ok(config) => return finish(config, ConfigSource::File, Vec::new()),
        Err(Error::Config(msg)) if msg.contains("newer Lumina") => {
            return Err(Error::Config(msg));
        }
        Err(err) => err,
    };

    tracing::error!("config.toml is invalid: {parse_error}");
    let corrupt = corrupt_path(path);
    std::fs::copy(path, &corrupt).map_err(|source| Error::Io {
        path: corrupt.clone(),
        source,
    })?;
    let mut notices = vec![Warning {
        code: "config_corrupt",
        field: String::new(),
        message: format!(
            "config.toml could not be read ({parse_error}); it was saved as {}",
            corrupt.display()
        ),
        values: [("file", corrupt.display().to_string())].into(),
    }];

    let backup = backup_path(path);
    if let Some(config) = std::fs::read_to_string(&backup)
        .ok()
        .and_then(|t| parse(&t).ok())
    {
        std::fs::copy(&backup, path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        notices[0].code = "config_restored";
        return finish(config, ConfigSource::RestoredFromBackup, notices);
    }

    lumina_core::fs::write_atomic(path, DEFAULT_TOML.as_bytes())?;
    notices[0].code = "config_reset";
    finish(parse(DEFAULT_TOML)?, ConfigSource::ResetToDefaults, notices)
}

/// First start of a different Lumina version: copies config.toml to
/// `config.toml.before-<version>.bak` before the new version reads (and maybe
/// migrates) it. Returns the copy, or `None` when there was nothing to copy.
pub fn backup_before_upgrade(path: &Path, version: &str) -> Result<Option<PathBuf>> {
    if !path.is_file() {
        return Ok(None);
    }
    let mut name = path.as_os_str().to_owned();
    name.push(format!(".before-{version}.bak"));
    let copy = PathBuf::from(name);
    if copy.exists() {
        // Already taken by an earlier start of this version.
        return Ok(Some(copy));
    }
    std::fs::copy(path, &copy).map_err(|source| Error::Io {
        path: copy.clone(),
        source,
    })?;
    Ok(Some(copy))
}

/// Saves `config`, keeping the previous file as `config.toml.bak`.
pub fn save(path: &Path, config: &AppConfig) -> Result<()> {
    let text = toml::to_string_pretty(config).map_err(|e| Error::Config(e.to_string()))?;
    if path.exists() {
        let backup = backup_path(path);
        std::fs::copy(path, &backup).map_err(|source| Error::Io {
            path: backup,
            source,
        })?;
    }
    lumina_core::fs::write_atomic(path, text.as_bytes())
}

/// Parses and migrates TOML text (no validation).
pub fn parse(text: &str) -> Result<AppConfig> {
    let mut value: toml::Value = toml::from_str(text).map_err(|e| Error::Config(e.to_string()))?;
    migrate(&mut value)?;
    value
        .try_into()
        .map_err(|e: toml::de::Error| Error::Config(e.to_string()))
}

fn finish(
    mut config: AppConfig,
    source: ConfigSource,
    mut warnings: Vec<Warning>,
) -> Result<Loaded> {
    warnings.extend(validate(&mut config));
    for w in &warnings {
        tracing::warn!("config: {}", w.message);
    }
    Ok(Loaded {
        config,
        warnings,
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("config.toml");
        (tmp, path)
    }

    #[test]
    fn profiles_capture_apply_and_survive_a_save() {
        use crate::schema::{DatabaseEngine, Profile, WebServer};
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("config.toml");
        let mut cfg = AppConfig::default();
        cfg.web.server = WebServer::Apache;
        cfg.web.apache_version = "2.4".into();
        cfg.php.version = "8.4".into();
        cfg.database.engine = DatabaseEngine::Mariadb;
        cfg.database.mariadb_version = "11.8".into();
        let modern = Profile::capture(&cfg, "Modern");
        assert!(modern.matches(&cfg));

        let legacy = Profile {
            name: "Legacy".into(),
            php: "8.3".into(),
            ..Profile::default()
        };
        legacy.apply(&mut cfg);
        assert!(legacy.matches(&cfg) && !modern.matches(&cfg));
        assert_eq!(
            (cfg.web.server, cfg.php.version.as_str()),
            (WebServer::Nginx, "8.3")
        );
        assert_eq!(
            cfg.web.apache_version, "2.4",
            "other servers' versions are kept"
        );
        assert_eq!(cfg.profiles.active, "Legacy");

        cfg.profiles.list = vec![legacy.clone(), modern.clone()];
        crate::save(&path, &cfg).expect("save");
        let text = std::fs::read_to_string(&path).expect("read");
        assert!(text.contains("[[profiles.list]]"), "{text}");
        let loaded = load_or_create(&path).expect("load").config;
        assert_eq!(loaded.profiles.list, vec![legacy, modern]);
    }

    #[test]
    fn default_template_matches_schema_defaults() {
        assert_eq!(parse(DEFAULT_TOML).expect("parse"), AppConfig::default());
    }

    #[test]
    fn creates_commented_default_file() {
        let (_tmp, path) = setup();
        let loaded = load_or_create(&path).expect("load");

        assert_eq!(loaded.source, ConfigSource::Created);
        assert_eq!(loaded.config, AppConfig::default());
        assert!(loaded.warnings.is_empty());
        let text = std::fs::read_to_string(&path).expect("read");
        assert!(text.contains("# Lumina configuration"));
    }

    #[test]
    fn an_upgrade_keeps_the_old_config() {
        let (_tmp, path) = setup();
        assert_eq!(backup_before_upgrade(&path, "1.0.0").expect("none"), None);
        std::fs::write(&path, "schema_version = 1\n# mine\n").expect("write");
        let copy = backup_before_upgrade(&path, "1.0.0")
            .expect("backup")
            .expect("copied");
        assert!(copy.ends_with("config.toml.before-1.0.0.bak"));
        assert_eq!(
            std::fs::read_to_string(&copy).expect("read"),
            "schema_version = 1\n# mine\n"
        );
        // A second start of the same version keeps the first copy.
        std::fs::write(&path, "changed").expect("write");
        backup_before_upgrade(&path, "1.0.0").expect("again");
        assert!(
            std::fs::read_to_string(&copy)
                .expect("read")
                .contains("# mine")
        );
    }

    #[test]
    fn round_trips_and_keeps_backup() {
        let (_tmp, path) = setup();
        load_or_create(&path).expect("create");

        let mut cfg = AppConfig::default();
        cfg.php.fcgi_children = 20;
        cfg.editor.command = "phpstorm".into();
        save(&path, &cfg).expect("save");

        let loaded = load_or_create(&path).expect("reload");
        assert_eq!(loaded.source, ConfigSource::File);
        assert_eq!(loaded.config, cfg);
        let backup = std::fs::read_to_string(backup_path(&path)).expect("backup");
        assert!(
            backup.contains("# Lumina configuration"),
            "backup is the previous file"
        );
    }

    #[test]
    fn partial_file_fills_in_defaults() {
        let (_tmp, path) = setup();
        std::fs::write(&path, "[php]\nfcgi_children = 2\n").expect("write");

        let loaded = load_or_create(&path).expect("load");

        assert_eq!(loaded.config.php.fcgi_children, 4);
        assert_eq!(loaded.config.php.fcgi_port, 10987);
        assert_eq!(loaded.warnings[0].code, "php_children_raised");
        // Repairs are in memory only; the user's file is untouched.
        assert!(
            std::fs::read_to_string(&path)
                .expect("read")
                .contains("= 2")
        );
    }

    #[test]
    fn corrupt_file_restores_backup() {
        let (_tmp, path) = setup();
        let mut cfg = AppConfig::default();
        cfg.php.fcgi_children = 16;
        save(&path, &cfg).expect("first save");
        save(&path, &cfg).expect("second save creates .bak");
        std::fs::write(&path, "[php\nbroken").expect("corrupt");

        let loaded = load_or_create(&path).expect("load");

        assert_eq!(loaded.source, ConfigSource::RestoredFromBackup);
        assert_eq!(loaded.config.php.fcgi_children, 16);
        assert_eq!(loaded.warnings[0].code, "config_restored");
        assert!(corrupt_path(&path).exists());
        assert_eq!(
            load_or_create(&path).expect("reload").source,
            ConfigSource::File
        );
    }

    #[test]
    fn corrupt_file_without_backup_resets_to_defaults() {
        let (_tmp, path) = setup();
        std::fs::write(&path, "php = = =").expect("corrupt");

        let loaded = load_or_create(&path).expect("load");

        assert_eq!(loaded.source, ConfigSource::ResetToDefaults);
        assert_eq!(loaded.config, AppConfig::default());
        assert_eq!(loaded.warnings[0].code, "config_reset");
    }

    #[test]
    fn wrong_types_count_as_corrupt() {
        let (_tmp, path) = setup();
        std::fs::write(&path, "[php]\nfcgi_children = \"many\"\n").expect("write");
        assert_eq!(
            load_or_create(&path).expect("load").source,
            ConfigSource::ResetToDefaults
        );
    }

    #[test]
    fn newer_schema_is_an_error_and_file_is_kept() {
        let (_tmp, path) = setup();
        std::fs::write(&path, "schema_version = 99\n").expect("write");

        assert!(load_or_create(&path).is_err());
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "schema_version = 99\n"
        );
    }
}
