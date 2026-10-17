//! Typed `config.toml`. Every section and field has a default, so partial or
//! older files load; unknown keys are ignored.
//!
//! Must stay in sync with resources/config/default.toml (a test enforces this).

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::defaults;

/// Bump when a change needs a migration step in `migrate.rs`.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(default)]
pub struct AppConfig {
    pub schema_version: u32,
    pub general: General,
    pub php: Php,
    pub web: Web,
    pub database: Database,
    pub redis: Redis,
    pub postgresql: Postgresql,
    pub mailpit: Mailpit,
    pub editor: Editor,
    pub tunnel: Tunnel,
    pub updates: Updates,
    pub profiles: Profiles,
    pub tools: Tools,
    pub packages: Packages,
    /// Per-site settings, keyed by the project's folder name.
    pub sites: BTreeMap<String, SiteOverride>,
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
            postgresql: Postgresql::default(),
            mailpit: Mailpit::default(),
            editor: Editor::default(),
            tunnel: Tunnel::default(),
            updates: Updates::default(),
            profiles: Profiles::default(),
            tools: Tools::default(),
            packages: Packages::default(),
            sites: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(default)]
pub struct General {
    /// Projects folder; `None` (empty in the file) means `<data-root>/www`.
    #[serde(with = "empty_path_as_none")]
    #[specta(type = String)]
    pub www_root: Option<PathBuf>,
    /// Domain suffix for automatic sites: folder `blog` becomes `blog.<tld>`.
    pub tld: String,
    /// Every folder in the projects folder gets a virtual host and a hosts entry.
    pub auto_vhosts: bool,
    /// HTTPS for localhost and every site, with certificates from Lumina's local CA.
    pub auto_ssl: bool,
    /// Sites answer plain HTTP with a redirect to HTTPS (needs `auto_ssl`).
    pub force_https: bool,
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
            force_https: false,
            start_on_login: false,
            autostart_services: false,
            close_to_tray: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum WebServer {
    Nginx,
    Apache,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseEngine {
    Mysql,
    Mariadb,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
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

/// PostgreSQL, a separate server next to MySQL/MariaDB (`[postgresql]`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(default)]
pub struct Postgresql {
    /// Start PostgreSQL with "Start All".
    pub enabled: bool,
    /// Empty = newest installed; `"17"` = newest 17.x.
    pub version: String,
    pub port: u16,
}

impl Default for Postgresql {
    fn default() -> Self {
        Self {
            enabled: false,
            version: String::new(),
            port: 5432,
        }
    }
}

/// Mailpit, which catches the mail PHP and your apps send (`[mailpit]`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(default)]
pub struct Mailpit {
    /// Start Mailpit with "Start All".
    pub enabled: bool,
    /// Empty = newest installed.
    pub version: String,
    /// Where apps send mail (SMTP).
    pub smtp_port: u16,
    /// The inbox in the browser.
    pub ui_port: u16,
    /// Send PHP's `mail()` to Mailpit instead of the `SMTP` / `sendmail_path`
    /// in php.ini.
    pub php_mail: bool,
}

impl Default for Mailpit {
    fn default() -> Self {
        Self {
            enabled: false,
            version: String::new(),
            smtp_port: 1025,
            ui_port: 8025,
            php_mail: true,
        }
    }
}

/// Versions of tools that are not services. Empty = newest installed; for
/// phpMyAdmin, empty = the newest one that runs on the active PHP.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(default)]
pub struct Tools {
    pub node: String,
    pub python: String,
    pub composer: String,
    pub phpmyadmin: String,
    pub ngrok: String,
}

/// Quick Add (downloads).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(default)]
pub struct Packages {
    /// `http://host:port` for downloads; empty = the system's proxy settings.
    pub proxy: String,
}

/// Settings for one automatic site (`[sites.<folder>]`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(default)]
pub struct SiteOverride {
    /// `false`: no virtual host or hosts entry for this folder.
    pub enabled: bool,
    /// Document root relative to the project folder, or absolute; empty =
    /// detect (`public/`, `web/`, `public_html/`, then the folder itself).
    pub docroot: String,
}

impl Default for SiteOverride {
    fn default() -> Self {
        Self {
            enabled: true,
            docroot: String::new(),
        }
    }
}

/// The user's code editor (blueprint: custom editor support).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(default)]
pub struct Editor {
    /// Display name, e.g. "Notepad++".
    pub name: String,
    /// Executable name or full path.
    pub command: String,
    /// Arguments; `{path}` is replaced with the folder or file to open.
    pub args: Vec<String>,
    /// Arguments to open a file at a line (`{path}`, `{line}`); empty = `args`.
    pub args_line: Vec<String>,
}

impl Default for Editor {
    /// Notepad++ on Windows (its installer does not add it to PATH; the
    /// launcher also looks in Program Files), VS Code elsewhere.
    fn default() -> Self {
        let strings = |a: &[&str]| a.iter().map(|s| (*s).to_owned()).collect();
        if cfg!(windows) {
            Self {
                name: "Notepad++".to_owned(),
                command: "notepad++".to_owned(),
                // A folder opens in the Folder as Workspace panel instead of
                // opening every file in it.
                args: strings(&["-openFoldersAsWorkspace", "{path}"]),
                args_line: strings(&["-n{line}", "{path}"]),
            }
        } else {
            Self {
                name: "VS Code".to_owned(),
                command: "code".to_owned(),
                args: strings(&["{path}"]),
                args_line: strings(&["--goto", "{path}:{line}"]),
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(default)]
pub struct Tunnel {
    pub provider: String,
    /// A reserved ngrok domain (`my-app.ngrok-free.app`) offered when sharing;
    /// empty = a random address each time. The authtoken is in the system
    /// keychain, never here.
    pub domain: String,
}

impl Default for Tunnel {
    fn default() -> Self {
        Self {
            provider: "ngrok".to_owned(),
            domain: String::new(),
        }
    }
}

/// App updates (signed packages from GitHub releases).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(default)]
pub struct Updates {
    /// `stable` or `beta` (pre-releases, earlier).
    pub channel: String,
    /// Look for a new version at start-up and once a day.
    pub check: bool,
}

pub const UPDATE_CHANNELS: [&str; 2] = ["stable", "beta"];

impl Default for Updates {
    fn default() -> Self {
        Self {
            channel: "stable".to_owned(),
            check: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(default)]
pub struct Profiles {
    /// Name of the profile last switched to.
    pub active: String,
    /// Saved stacks, switched in one click.
    pub list: Vec<Profile>,
}

/// A saved stack (`[[profiles.list]]`). Versions use the same rules as the
/// sections they set: empty = newest installed, `"8.3"` = newest 8.3.x.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(default)]
pub struct Profile {
    pub name: String,
    pub web: WebServer,
    pub web_version: String,
    pub php: String,
    pub database: ProfileDatabase,
    /// Run Redis with this stack.
    pub redis: bool,
    /// Run PostgreSQL with this stack.
    pub postgresql: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(default)]
pub struct ProfileDatabase {
    pub engine: DatabaseEngine,
    pub version: String,
}

impl Default for ProfileDatabase {
    fn default() -> Self {
        Self {
            engine: DatabaseEngine::Mysql,
            version: String::new(),
        }
    }
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            name: String::new(),
            web: WebServer::Nginx,
            web_version: String::new(),
            php: String::new(),
            database: ProfileDatabase::default(),
            redis: false,
            postgresql: false,
        }
    }
}

impl Profile {
    /// The stack `cfg` describes now, named `name`.
    pub fn capture(cfg: &AppConfig, name: &str) -> Self {
        let engine = cfg.database.engine;
        Self {
            name: name.trim().to_owned(),
            web: cfg.web.server,
            web_version: match cfg.web.server {
                WebServer::Nginx => cfg.web.nginx_version.clone(),
                WebServer::Apache => cfg.web.apache_version.clone(),
            },
            php: cfg.php.version.clone(),
            database: ProfileDatabase {
                engine,
                version: match engine {
                    DatabaseEngine::Mysql => cfg.database.mysql_version.clone(),
                    DatabaseEngine::Mariadb => cfg.database.mariadb_version.clone(),
                },
            },
            redis: cfg.redis.enabled,
            postgresql: cfg.postgresql.enabled,
        }
    }

    /// Sets the sections this profile covers; ports and everything else stay.
    pub fn apply(&self, cfg: &mut AppConfig) {
        cfg.web.server = self.web;
        match self.web {
            WebServer::Nginx => cfg.web.nginx_version = self.web_version.clone(),
            WebServer::Apache => cfg.web.apache_version = self.web_version.clone(),
        }
        cfg.php.version = self.php.clone();
        cfg.database.engine = self.database.engine;
        match self.database.engine {
            DatabaseEngine::Mysql => cfg.database.mysql_version = self.database.version.clone(),
            DatabaseEngine::Mariadb => cfg.database.mariadb_version = self.database.version.clone(),
        }
        cfg.redis.enabled = self.redis;
        cfg.postgresql.enabled = self.postgresql;
        cfg.profiles.active = self.name.clone();
    }

    /// Whether `cfg` runs exactly this stack.
    pub fn matches(&self, cfg: &AppConfig) -> bool {
        let mut now = Self::capture(cfg, &self.name);
        now.name = self.name.clone();
        now == *self
    }
}

impl Default for Profiles {
    fn default() -> Self {
        Self {
            active: "default".to_owned(),
            list: Vec::new(),
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
