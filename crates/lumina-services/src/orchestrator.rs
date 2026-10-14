//! Starts and stops the services as one stack.
//!
//! - Order: PHP first, then the web server; stopping goes the other way.
//! - One operation at a time: concurrent UI clicks and tray actions are queued.
//! - Only one web server runs: starting nginx stops Apache and vice versa (they
//!   would fight over port 80).
//! - PHP's actual port (which may be the fallback) is pushed into the web
//!   server's config; if PHP moves, a running web server is reloaded.

use std::path::PathBuf;
use std::sync::Arc;

use lumina_config::AppConfig;
use lumina_config::schema::WebServer;
use lumina_core::paths::Layout;
use lumina_core::types::{ServiceKind, ServiceStatus};
use lumina_core::{Error, Result};
use lumina_process::Supervisor;
use serde::Serialize;
use tokio::sync::Mutex;

use crate::apache::Apache;
use crate::nginx::Nginx;
use crate::php_fcgi::{PhpFcgi, PhpStatus};
use crate::template::Templates;
use crate::web::{ReloadOutcome, WebContext, WebStatus};

/// Everything the Services page shows.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StackStatus {
    pub php: PhpStatus,
    /// The configured web server, or the other one if that is what runs.
    pub web: WebStatus,
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
    /// Serialises start/stop/reload so the stack never sees two at once.
    lock: Mutex<()>,
}

impl Orchestrator {
    pub fn new(supervisor: Supervisor, layout: Layout, templates: Templates) -> Self {
        Self {
            inner: Arc::new(Inner {
                php: PhpFcgi::new(supervisor.clone(), layout.clone(), templates.clone()),
                nginx: Nginx::new(supervisor.clone(), layout.clone(), templates.clone()),
                apache: Apache::new(supervisor, layout.clone(), templates),
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
        WebContext {
            http_port: cfg.web.http_port,
            php_port,
            www_root: self.www_root(cfg),
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
            other => Err(not_available(other)),
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
            other => Err(not_available(other)),
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

    /// PHP, then the configured web server. Both are attempted; the first error
    /// is returned.
    pub async fn start_all(&self, cfg: &AppConfig) -> Result<()> {
        let _guard = self.inner.lock.lock().await;
        let php = if self.inner.php.status().await.runtime.is_none() {
            self.start_locked(ServiceKind::PhpFcgi, cfg).await
        } else {
            Ok(())
        };
        let web_kind = Self::configured_web(cfg);
        let web = if self.web_status(web_kind).await.runtime.is_none() {
            self.start_locked(web_kind, cfg).await
        } else {
            Ok(())
        };
        php.and(web)
    }

    /// Web servers first, then PHP. Returns the services that failed to stop.
    pub async fn stop_all(&self) -> Vec<(ServiceKind, Error)> {
        let _guard = self.inner.lock.lock().await;
        let mut failures = Vec::new();
        for kind in [
            ServiceKind::Nginx,
            ServiceKind::Apache,
            ServiceKind::PhpFcgi,
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
        StackStatus {
            php: self.inner.php.status().await,
            web,
        }
    }
}

fn not_available(kind: ServiceKind) -> Error {
    Error::Service(
        format!("{kind:?}"),
        "not available yet (arrives in a later phase)".into(),
    )
}
