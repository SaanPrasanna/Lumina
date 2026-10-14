//! Starts and stops the services as one stack.
//!
//! - Order: database, Redis, PHP, then the web server; stopping goes the other
//!   way, so nothing is cut off from a service it depends on.
//! - One operation at a time: concurrent UI clicks and tray actions are queued.
//! - Only one web server and one database engine run: starting nginx stops
//!   Apache (port 80), starting MariaDB stops MySQL (port 3306), and vice versa.
//! - PHP's actual port (which may be the fallback) is pushed into the web
//!   server's config; if PHP moves, a running web server is reloaded.

use std::path::PathBuf;
use std::sync::Arc;

use lumina_config::AppConfig;
use lumina_config::schema::{DatabaseEngine, WebServer};
use lumina_core::paths::Layout;
use lumina_core::types::{ServiceKind, ServiceStatus};
use lumina_core::{Error, Result};
use lumina_process::Supervisor;
use serde::Serialize;
use tokio::sync::Mutex;

use crate::apache::Apache;
use crate::database::{Database, DbStatus, engine};
use crate::nginx::Nginx;
use crate::php_fcgi::{PhpFcgi, PhpStatus};
use crate::redis::{Redis, RedisStatus};
use crate::template::Templates;
use crate::web::{ReloadOutcome, WebContext, WebStatus};
use lumina_vhost::Scan;

/// Everything the Services page shows.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StackStatus {
    pub php: PhpStatus,
    /// The configured web server, or the other one if that is what runs.
    pub web: WebStatus,
    /// The configured database engine, or the other one if that is what runs.
    pub database: DbStatus,
    pub redis: RedisStatus,
}

#[derive(Clone)]
pub struct Orchestrator {
    inner: Arc<Inner>,
}

struct Inner {
    layout: Layout,
    php: PhpFcgi,
    nginx: Nginx,
    apache: Apache,
    mysql: Database,
    mariadb: Database,
    redis: Redis,
    /// Serialises start/stop/reload so the stack never sees two at once.
    lock: Mutex<()>,
}

impl Orchestrator {
    pub fn new(supervisor: Supervisor, layout: Layout, templates: Templates) -> Self {
        Self {
            inner: Arc::new(Inner {
                php: PhpFcgi::new(supervisor.clone(), layout.clone(), templates.clone()),
                nginx: Nginx::new(supervisor.clone(), layout.clone(), templates.clone()),
                apache: Apache::new(supervisor.clone(), layout.clone(), templates.clone()),
                mysql: Database::new(
                    engine::MYSQL,
                    supervisor.clone(),
                    layout.clone(),
                    templates.clone(),
                ),
                mariadb: Database::new(
                    engine::MARIADB,
                    supervisor.clone(),
                    layout.clone(),
                    templates.clone(),
                ),
                redis: Redis::new(supervisor, layout.clone(), templates),
                layout,
                lock: Mutex::new(()),
            }),
        }
    }

    pub fn php(&self) -> &PhpFcgi {
        &self.inner.php
    }

    pub fn nginx(&self) -> &Nginx {
        &self.inner.nginx
    }

    pub fn apache(&self) -> &Apache {
        &self.inner.apache
    }

    pub fn database(&self, engine: DatabaseEngine) -> &Database {
        match engine {
            DatabaseEngine::Mysql => &self.inner.mysql,
            DatabaseEngine::Mariadb => &self.inner.mariadb,
        }
    }

    pub fn redis(&self) -> &Redis {
        &self.inner.redis
    }

    /// The service kind of the configured database engine.
    pub fn configured_db(cfg: &AppConfig) -> ServiceKind {
        match cfg.database.engine {
            DatabaseEngine::Mysql => ServiceKind::Mysql,
            DatabaseEngine::Mariadb => ServiceKind::Mariadb,
        }
    }

    /// Served at http://localhost: `general.www_root`, or `<data-root>/www`.
    pub fn www_root(&self, cfg: &AppConfig) -> PathBuf {
        cfg.general
            .www_root
            .clone()
            .unwrap_or_else(|| self.inner.layout.www())
    }

    /// The service kind of the configured web server.
    pub fn configured_web(cfg: &AppConfig) -> ServiceKind {
        match cfg.web.server {
            WebServer::Nginx => ServiceKind::Nginx,
            WebServer::Apache => ServiceKind::Apache,
        }
    }

    async fn web_context(&self, cfg: &AppConfig) -> WebContext {
        let php_port = self
            .inner
            .php
            .status()
            .await
            .runtime
            .map_or(cfg.php.fcgi_port, |rt| rt.port);
        let www_root = self.www_root(cfg);
        WebContext {
            http_port: cfg.web.http_port,
            php_port,
            sites: lumina_vhost::scan(&www_root, cfg).sites,
            www_root,
        }
    }

    /// The automatic sites for the projects folder as it is now.
    pub fn sites(&self, cfg: &AppConfig) -> Scan {
        lumina_vhost::scan(&self.www_root(cfg), cfg)
    }

    /// The web server that is running, if any.
    pub async fn running_web(&self) -> Option<ServiceKind> {
        for kind in [ServiceKind::Nginx, ServiceKind::Apache] {
            if self.web_status(kind).await.runtime.is_some() {
                return Some(kind);
            }
        }
        None
    }

    /// Brings a running web server's virtual hosts in line with the projects
    /// folder (one reload, only if a vhost file changed). `None` if no web
    /// server runs; its sites are rendered when it starts.
    pub async fn sync_sites(&self, cfg: &AppConfig) -> Result<Option<ReloadOutcome>> {
        let _guard = self.inner.lock.lock().await;
        match self.running_web().await {
            Some(kind) => self.reload_locked(kind, cfg).await.map(Some),
            None => Ok(None),
        }
    }

    pub async fn start(&self, kind: ServiceKind, cfg: &AppConfig) -> Result<()> {
        let _guard = self.inner.lock.lock().await;
        self.start_locked(kind, cfg).await
    }

    async fn start_locked(&self, kind: ServiceKind, cfg: &AppConfig) -> Result<()> {
        match kind {
            ServiceKind::PhpFcgi => {
                self.inner.php.start(&cfg.php).await?;
                self.sync_web_to_php(cfg).await
            }
            ServiceKind::Nginx | ServiceKind::Apache => {
                // The web server needs PHP; start it first if it is down.
                if self.inner.php.status().await.runtime.is_none() {
                    self.inner.php.start(&cfg.php).await?;
                }
                let other = if kind == ServiceKind::Nginx {
                    ServiceKind::Apache
                } else {
                    ServiceKind::Nginx
                };
                if self.web_status(other).await.runtime.is_some() {
                    self.stop_locked(other).await?;
                }
                let ctx = self.web_context(cfg).await;
                match kind {
                    ServiceKind::Nginx => self.inner.nginx.start(&cfg.web, &ctx).await?,
                    _ => self.inner.apache.start(&cfg.web, &ctx).await?,
                };
                Ok(())
            }
            ServiceKind::Mysql | ServiceKind::Mariadb => {
                let (this, other) = if kind == ServiceKind::Mysql {
                    (&self.inner.mysql, &self.inner.mariadb)
                } else {
                    (&self.inner.mariadb, &self.inner.mysql)
                };
                if other.status().await.runtime.is_some() {
                    other.stop().await?;
                }
                this.start(&cfg.database).await?;
                Ok(())
            }
            ServiceKind::Redis => {
                self.inner.redis.start(&cfg.redis).await?;
                Ok(())
            }
        }
    }

    pub async fn stop(&self, kind: ServiceKind) -> Result<()> {
        let _guard = self.inner.lock.lock().await;
        self.stop_locked(kind).await
    }

    async fn stop_locked(&self, kind: ServiceKind) -> Result<()> {
        match kind {
            ServiceKind::PhpFcgi => self.inner.php.stop().await,
            ServiceKind::Nginx => self.inner.nginx.stop().await,
            ServiceKind::Apache => self.inner.apache.stop().await,
            ServiceKind::Mysql => self.inner.mysql.stop().await,
            ServiceKind::Mariadb => self.inner.mariadb.stop().await,
            ServiceKind::Redis => self.inner.redis.stop().await,
        }
    }

    pub async fn restart(&self, kind: ServiceKind, cfg: &AppConfig) -> Result<()> {
        let _guard = self.inner.lock.lock().await;
        match kind {
            ServiceKind::PhpFcgi => {
                self.inner.php.restart(&cfg.php).await?;
                self.sync_web_to_php(cfg).await
            }
            _ => {
                self.stop_locked(kind).await?;
                self.start_locked(kind, cfg).await
            }
        }
    }

    /// Re-renders a running web server's config and applies it if it changed.
    pub async fn reload(&self, kind: ServiceKind, cfg: &AppConfig) -> Result<ReloadOutcome> {
        let _guard = self.inner.lock.lock().await;
        self.reload_locked(kind, cfg).await
    }

    async fn reload_locked(&self, kind: ServiceKind, cfg: &AppConfig) -> Result<ReloadOutcome> {
        let ctx = self.web_context(cfg).await;
        match kind {
            ServiceKind::Nginx => self.inner.nginx.reload(&ctx).await,
            ServiceKind::Apache => self.inner.apache.reload(&cfg.web, &ctx).await,
            other => Err(Error::Service(
                format!("{other:?}"),
                "only web servers can be reloaded".into(),
            )),
        }
    }

    /// After PHP (re)starts, point a running web server at PHP's actual port.
    async fn sync_web_to_php(&self, cfg: &AppConfig) -> Result<()> {
        for kind in [ServiceKind::Nginx, ServiceKind::Apache] {
            if self.web_status(kind).await.runtime.is_some() {
                self.reload_locked(kind, cfg).await?;
            }
        }
        Ok(())
    }

    /// Database, Redis (if enabled), PHP, then the configured web server. Every
    /// service is attempted; the first error is returned.
    pub async fn start_all(&self, cfg: &AppConfig) -> Result<()> {
        let _guard = self.inner.lock.lock().await;
        let mut kinds = vec![Self::configured_db(cfg)];
        if cfg.redis.enabled {
            kinds.push(ServiceKind::Redis);
        }
        kinds.push(ServiceKind::PhpFcgi);
        kinds.push(Self::configured_web(cfg));

        let mut first_error = None;
        for kind in kinds {
            if self.is_running(kind).await {
                continue;
            }
            if let Err(err) = self.start_locked(kind, cfg).await {
                tracing::error!("could not start {kind:?}: {err}");
                first_error.get_or_insert(err);
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    async fn is_running(&self, kind: ServiceKind) -> bool {
        match kind {
            ServiceKind::PhpFcgi => self.inner.php.status().await.runtime.is_some(),
            ServiceKind::Nginx | ServiceKind::Apache => {
                self.web_status(kind).await.runtime.is_some()
            }
            ServiceKind::Mysql => self.inner.mysql.status().await.runtime.is_some(),
            ServiceKind::Mariadb => self.inner.mariadb.status().await.runtime.is_some(),
            ServiceKind::Redis => self.inner.redis.status().await.runtime.is_some(),
        }
    }

    /// Web servers, PHP, Redis, then databases. Returns the services that
    /// failed to stop.
    pub async fn stop_all(&self) -> Vec<(ServiceKind, Error)> {
        let _guard = self.inner.lock.lock().await;
        let mut failures = Vec::new();
        for kind in [
            ServiceKind::Nginx,
            ServiceKind::Apache,
            ServiceKind::PhpFcgi,
            ServiceKind::Redis,
            ServiceKind::Mysql,
            ServiceKind::Mariadb,
        ] {
            if let Err(err) = self.stop_locked(kind).await {
                tracing::error!("could not stop {kind:?}: {err}");
                failures.push((kind, err));
            }
        }
        failures
    }

    pub async fn web_status(&self, kind: ServiceKind) -> WebStatus {
        match kind {
            ServiceKind::Apache => self.inner.apache.status().await,
            _ => self.inner.nginx.status().await,
        }
    }

    pub async fn status(&self, cfg: &AppConfig) -> StackStatus {
        let configured = Self::configured_web(cfg);
        let mut web = self.web_status(configured).await;
        if web.status == ServiceStatus::Stopped {
            // Show the other server if that is the one actually running.
            let other = if configured == ServiceKind::Nginx {
                ServiceKind::Apache
            } else {
                ServiceKind::Nginx
            };
            let other_status = self.web_status(other).await;
            if other_status.runtime.is_some() {
                web = other_status;
            }
        }
        let (configured_db, other_db) = match cfg.database.engine {
            DatabaseEngine::Mysql => (&self.inner.mysql, &self.inner.mariadb),
            DatabaseEngine::Mariadb => (&self.inner.mariadb, &self.inner.mysql),
        };
        let mut database = configured_db.status().await;
        if database.status == ServiceStatus::Stopped {
            let other = other_db.status().await;
            if other.runtime.is_some() {
                database = other;
            }
        }
        StackStatus {
            php: self.inner.php.status().await,
            web,
            database,
            redis: self.inner.redis.status().await,
        }
    }
}
