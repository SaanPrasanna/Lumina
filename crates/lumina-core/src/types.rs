//! Core domain types shared between backend crates and (via serde) the UI.

use serde::{Deserialize, Serialize};

/// Every service Lumina can manage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ServiceKind {
    PhpFcgi,
    Nginx,
    Apache,
    Mysql,
    Mariadb,
    Redis,
}

/// Lifecycle state reported to the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ServiceStatus {
    Stopped,
    Starting,
    Running,
    Stopping,
    Failed,
}
