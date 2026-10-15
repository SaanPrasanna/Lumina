//! Saved stacks (e.g. Nginx + PHP 8.3 vs Apache + PHP 8.4).
//!
//! Switching only touches what differs: [`plan`] compares the running
//! services with the new configuration and lists what to stop (web server
//! first, database last) and what to start (database first, web server last).
//! Services that are not running stay stopped; unchanged ones keep running.
//! The orchestrator executes the plan and rolls back on failure.

use lumina_config::AppConfig;
use lumina_config::schema::{DatabaseEngine, WebServer};
use lumina_core::types::ServiceKind;
use serde::Serialize;

/// What runs right now, one entry per kind of service.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Running {
    pub php: bool,
    pub web: Option<ServiceKind>,
    pub database: Option<ServiceKind>,
    pub redis: bool,
}

impl Running {
    fn any(&self) -> bool {
        self.php || self.web.is_some() || self.database.is_some() || self.redis
    }
}

/// The steps of a switch.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    /// In this order: web, PHP, Redis, database.
    pub stop: Vec<ServiceKind>,
    /// In this order: database, Redis, PHP, web.
    pub start: Vec<ServiceKind>,
    /// Running and unchanged.
    pub keep: Vec<ServiceKind>,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.stop.is_empty() && self.start.is_empty()
    }
}

fn web_kind(server: WebServer) -> ServiceKind {
    match server {
        WebServer::Nginx => ServiceKind::Nginx,
        WebServer::Apache => ServiceKind::Apache,
    }
}

fn db_kind(engine: DatabaseEngine) -> ServiceKind {
    match engine {
        DatabaseEngine::Mysql => ServiceKind::Mysql,
        DatabaseEngine::Mariadb => ServiceKind::Mariadb,
    }
}

fn web_version(cfg: &AppConfig, kind: ServiceKind) -> &str {
    match kind {
        ServiceKind::Apache => &cfg.web.apache_version,
        _ => &cfg.web.nginx_version,
    }
}

fn db_version(cfg: &AppConfig, kind: ServiceKind) -> &str {
    match kind {
        ServiceKind::Mariadb => &cfg.database.mariadb_version,
        _ => &cfg.database.mysql_version,
    }
}

/// What to stop and start to go from `old` (with `running`) to `new`.
pub fn plan(running: &Running, old: &AppConfig, new: &AppConfig) -> Plan {
    let mut stop = Vec::new();
    let mut start = Vec::new();
    let mut keep = Vec::new();

    // Database
    let mut db_start = None;
    if let Some(kind) = running.database {
        let next = db_kind(new.database.engine);
        if kind != next || db_version(old, kind) != db_version(new, next) {
            stop.push(kind);
            db_start = Some(next);
        } else {
            keep.push(kind);
        }
    }

    // Redis: follows `redis.enabled` while the stack runs.
    let mut redis_start = false;
    if running.redis {
        if !new.redis.enabled {
            stop.push(ServiceKind::Redis);
        } else if old.redis.version != new.redis.version {
            stop.push(ServiceKind::Redis);
            redis_start = true;
        } else {
            keep.push(ServiceKind::Redis);
        }
    } else if new.redis.enabled && running.any() {
        redis_start = true;
    }

    // PHP
    let mut php_start = false;
    if running.php {
        if old.php.version != new.php.version {
            stop.push(ServiceKind::PhpFcgi);
            php_start = true;
        } else {
            keep.push(ServiceKind::PhpFcgi);
        }
    }

    // Web server
    let mut web_start = None;
    if let Some(kind) = running.web {
        let next = web_kind(new.web.server);
        if kind != next || web_version(old, kind) != web_version(new, next) {
            stop.push(kind);
            web_start = Some(next);
        } else {
            keep.push(kind);
        }
    }

    // Stop dependants first: web, PHP, Redis, database.
    let order = |k: &ServiceKind| match k {
        ServiceKind::Nginx | ServiceKind::Apache => 0,
        ServiceKind::PhpFcgi => 1,
        ServiceKind::Redis => 2,
        ServiceKind::Mysql | ServiceKind::Mariadb => 3,
    };
    stop.sort_by_key(order);

    start.extend(db_start);
    if redis_start {
        start.push(ServiceKind::Redis);
    }
    if php_start {
        start.push(ServiceKind::PhpFcgi);
    }
    start.extend(web_start);

    Plan { stop, start, keep }
}

#[cfg(test)]
mod tests {
    use lumina_config::schema::Profile;

    use super::*;

    fn stack(web: WebServer, php: &str) -> AppConfig {
        let mut cfg = AppConfig::default();
        Profile {
            name: "x".into(),
            web,
            php: php.into(),
            ..Profile::default()
        }
        .apply(&mut cfg);
        cfg
    }

    fn all_running(cfg: &AppConfig) -> Running {
        Running {
            php: true,
            web: Some(web_kind(cfg.web.server)),
            database: Some(db_kind(cfg.database.engine)),
            redis: cfg.redis.enabled,
        }
    }

    #[test]
    fn only_what_changed_is_restarted_in_dependency_order() {
        let old = stack(WebServer::Nginx, "8.3");
        let new = stack(WebServer::Apache, "8.4");
        let p = plan(&all_running(&old), &old, &new);
        assert_eq!(p.stop, [ServiceKind::Nginx, ServiceKind::PhpFcgi]);
        assert_eq!(p.start, [ServiceKind::PhpFcgi, ServiceKind::Apache]);
        assert_eq!(p.keep, [ServiceKind::Mysql]);
    }

    #[test]
    fn same_stack_needs_nothing_and_stopped_services_stay_stopped() {
        let cfg = stack(WebServer::Nginx, "8.4");
        assert!(plan(&all_running(&cfg), &cfg, &cfg).is_empty());

        let new = stack(WebServer::Apache, "8.3");
        assert_eq!(plan(&Running::default(), &cfg, &new), Plan::default());

        // Only PHP runs: the web server change waits for the next start.
        let php_only = Running {
            php: true,
            ..Running::default()
        };
        let p = plan(&php_only, &cfg, &new);
        assert_eq!(
            (p.stop.as_slice(), p.start.as_slice()),
            (&[ServiceKind::PhpFcgi][..], &[ServiceKind::PhpFcgi][..])
        );
    }

    #[test]
    fn database_engine_and_redis_follow_the_profile() {
        let old = stack(WebServer::Nginx, "8.4");
        let mut new = old.clone();
        new.database.engine = DatabaseEngine::Mariadb;
        new.redis.enabled = true;
        let p = plan(&all_running(&old), &old, &new);
        assert_eq!(p.stop, [ServiceKind::Mysql]);
        assert_eq!(p.start, [ServiceKind::Mariadb, ServiceKind::Redis]);

        // Redis switched off by the profile is stopped.
        let mut running = all_running(&new);
        running.database = Some(ServiceKind::Mariadb);
        let p = plan(&running, &new, &old);
        assert_eq!(p.stop, [ServiceKind::Redis, ServiceKind::Mariadb]);
        assert_eq!(p.start, [ServiceKind::Mysql]);
    }
}
