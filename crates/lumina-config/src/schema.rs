//! Typed `config.toml`. Every section and field has a default, so partial or
//! older files load; unknown keys are ignored.
//!
//! Must stay in sync with resources/config/default.toml (a test enforces this).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::defaults;

/// Bump when a change needs a migration step in `migrate.rs`.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub schema_version: u32,
    pub general: General,
    pub php: Php,
    pub web: Web,
    pub database: Database,
    pub redis: Redis,
    pub editor: Editor,
    pub tunnel: Tunnel,
    pub profiles: Profiles,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            general: General::default(),
            php: Php::default(),
            web: Web::default(),
            database: Database::default(),
            redis: Redis::default(),
            editor: Editor::default(),
            tunnel: Tunnel::default(),
            profiles: Profiles::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct General {
    /// Projects folder; `None` (empty in the file) means `<data-root>/www`.
    #[serde(with = "empty_path_as_none")]
    pub www_root: Option<PathBuf>,
    pub tld: String,
    pub auto_vhosts: bool,
    pub auto_ssl: bool,
    pub start_on_login: bool,
    /// Start PHP and the web server when Lumina opens.
    pub autostart_services: bool,
    /// Closing the window hides Lumina in the tray instead of quitting.
    pub close_to_tray: bool,
}

impl Default for General {
    fn default() -> Self {
        Self {
            www_root: None,
            tld: defaults::TLD.to_owned(),
            auto_vhosts: true,
            auto_ssl: true,
            start_on_login: false,
            autostart_services: false,
            close_to_tray: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Php {
    /// Empty = newest installed; `"8.4"` = newest 8.4.x; `"8.4.13"` = exact.
    pub version: String,
    pub fcgi_port: u16,
    pub fcgi_fallback_port: u16,
    /// `PHP_FCGI_CHILDREN`; values below the minimum are raised by `validate`.
    pub fcgi_children: u32,
    /// `PHP_FCGI_MAX_REQUESTS`; 0 = unlimited.
    pub fcgi_max_requests: u32,
}

impl Default for Php {
    fn default() -> Self {
        Self {
            version: String::new(),
            fcgi_port: defaults::PHP_FCGI_PORT,
            fcgi_fallback_port: defaults::PHP_FCGI_FALLBACK_PORT,
            fcgi_children: defaults::PHP_FCGI_CHILDREN,
            fcgi_max_requests: defaults::PHP_FCGI_MAX_REQUESTS,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WebServer {
    Nginx,
    Apache,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Web {
    pub server: WebServer,
    /// Empty = newest installed; otherwise a version or prefix, as for PHP.
    pub nginx_version: String,
    pub apache_version: String,
    pub http_port: u16,
    pub https_port: u16,
}

impl Default for Web {
    fn default() -> Self {
        Self {
            server: WebServer::Nginx,
            nginx_version: String::new(),
            apache_version: String::new(),
            http_port: 80,
            https_port: 443,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseEngine {
    Mysql,
    Mariadb,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Database {
    pub engine: DatabaseEngine,
    /// Empty = newest installed; otherwise a version or prefix, as for PHP.
    pub mysql_version: String,
    pub mariadb_version: String,
    pub port: u16,
}

impl Default for Database {
    fn default() -> Self {
        Self {
            engine: DatabaseEngine::Mysql,
            mysql_version: String::new(),
            mariadb_version: String::new(),
            port: 3306,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Redis {
    /// Start Redis with "Start All".
    pub enabled: bool,
    /// Empty = newest installed.
    pub version: String,
    pub port: u16,
}

impl Default for Redis {
    fn default() -> Self {
        Self {
            enabled: false,
            version: String::new(),
            port: 6379,
        }
    }
}

/// The user's code editor (blueprint: custom editor support).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Editor {
    /// Display name, e.g. "VS Code".
    pub name: String,
    /// Executable name or full path.
    pub command: String,
    /// Arguments; `{path}` is replaced with the folder or file to open.
    pub args: Vec<String>,
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            name: "VS Code".to_owned(),
            command: "code".to_owned(),
            args: vec!["{path}".to_owned()],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tunnel {
    pub provider: String,
}

impl Default for Tunnel {
    fn default() -> Self {
        Self {
            provider: "ngrok".to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profiles {
    pub active: String,
}

impl Default for Profiles {
    fn default() -> Self {
        Self {
            active: "default".to_owned(),
        }
    }
}

/// TOML has no null, so an optional path is written as `""` when unset.
mod empty_path_as_none {
    use std::path::PathBuf;

    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(value: &Option<PathBuf>, s: S) -> Result<S::Ok, S::Error> {
        match value {
            Some(path) => s.serialize_str(&path.to_string_lossy()),
            None => s.serialize_str(""),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<PathBuf>, D::Error> {
        let raw = String::deserialize(d)?;
        Ok((!raw.trim().is_empty()).then(|| PathBuf::from(raw)))
    }
}
