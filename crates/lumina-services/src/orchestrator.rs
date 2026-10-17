//! Starts and stops the services as one stack.
//!
//! - Order: database, Redis, PHP, then the web server; stopping goes the other
//!   way, so nothing is cut off from a service it depends on.
//! - One operation at a time: concurrent UI clicks and tray actions are queued.
//! - Only one web server and one database engine run: starting nginx stops
//!   Apache (port 80), starting MariaDB stops MySQL (port 3306), and vice versa.
//! - PHP's actual port (which may be the fallback) is pushed into the web
//!   server's config; if PHP moves, a running web server is reloaded.
//! - [`Orchestrator::heal`] restarts a service that runs but no longer
//!   answers on its port (checked after the computer wakes from sleep).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

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
use crate::mailpit::{Mailpit, MailpitStatus};
use crate::nginx::Nginx;
use crate::php_fcgi::{PhpFcgi, PhpStatus};
use crate::postgresql::{PgStatus, Postgresql};
use crate::redis::{Redis, RedisStatus};
use crate::template::Templates;
use crate::web::{DbUi, ReloadOutcome, WebContext, WebStatus};
use lumina_ssl::{Ssl, Tls};
use lumina_vhost::Scan;

/// This computer's name, for the CA's name ("Lumina Local CA - DEV-PC").
pub fn host_name() -> String {
    sysinfo::System::host_name().unwrap_or_default()
}

/// Everything the Services page shows.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct StackStatus {
    pub php: PhpStatus,
    /// The configured web server, or the other one if that is what runs.
    pub web: WebStatus,
    /// The configured database engine, or the other one if that is what runs.
    pub database: DbStatus,
    pub redis: RedisStatus,
    pub postgresql: PgStatus,
    pub mailpit: MailpitStatus,
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
    postgresql: Postgresql,
    mailpit: Mailpit,
    /// The local CA and site certificates (`<data-root>/ssl`).
    ssl: Ssl,
    /// Why HTTPS could not be set up last time, for the UI.
    tls_error: std::sync::Mutex<Option<String>>,
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
                postgresql: Postgresql::new(supervisor.clone(), layout.clone()),
                mailpit: Mailpit::new(supervisor.clone(), layout.clone()),
                redis: Redis::new(supervisor, layout.clone(), templates),
                ssl: Ssl::new(layout.ssl()),
                tls_error: std::sync::Mutex::new(None),
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

    pub fn postgresql(&self) -> &Postgresql {
        &self.inner.postgresql
    }

    pub fn mailpit(&self) -> &Mailpit {
        &self.inner.mailpit
    }

    /// php.ini settings that send PHP's `mail()` to Mailpit (see
    /// [`crate::mailpit::php_ini`]), for php-cgi and the `php` command alike.
    pub async fn php_mail_ini(&self, cfg: &AppConfig) -> Vec<(String, String)> {
        if !cfg.mailpit.php_mail {
            return Vec::new();
        }
        let exe = self
            .inner
            .mailpit
            .selected(&cfg.mailpit)
            .await
            .map(|i| i.exe);
        crate::mailpit::php_ini(&cfg.mailpit, exe.as_deref())
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
        let sites = lumina_vhost::scan(&www_root, cfg).sites;
        let tls = self.tls(cfg, &sites);
        let db_ui = self.db_ui(cfg).await;
        WebContext {
            http_port: cfg.web.http_port,
            php_port,
            sites,
            www_root,
            tls,
            db_ui,
        }
    }

    /// The phpMyAdmin to serve: `tools.phpmyadmin` if set and installed, else
    /// the newest installed one whose PHP requirement the active PHP meets.
    pub async fn db_ui(&self, cfg: &AppConfig) -> Option<DbUi> {
        let php = self.inner.php.active_version(&cfg.php).await?;
        let installs =
            lumina_packages::installed::list(&self.inner.layout.bin().join("phpmyadmin"));
        let pinned = cfg.tools.phpmyadmin.trim();
        let chosen = if pinned.is_empty() {
            lumina_packages::compat::newest_for_php(&installs, &php)
        } else {
            installs
                .iter()
                .find(|i| i.version == pinned || i.version.starts_with(&format!("{pinned}.")))
        }?;
        Some(DbUi {
            version: chosen.version.clone(),
            dir: chosen.dir.clone(),
            php_version: php,
            secret: self.db_ui_secret(),
            db_port: cfg.database.port,
            tmp_dir: self.inner.layout.tmp().join("phpmyadmin"),
        })
    }

    /// phpMyAdmin's cookie-encryption secret, created once and kept in
    /// `etc/phpmyadmin/blowfish_secret` so sessions survive restarts.
    fn db_ui_secret(&self) -> String {
        let path = self
            .inner
            .layout
            .etc()
            .join("phpmyadmin")
            .join("blowfish_secret");
        if let Ok(secret) = std::fs::read_to_string(&path)
            && secret.trim().len() == 32
        {
            return secret.trim().to_owned();
        }
        let secret = lumina_ssl::random_token(32);
        if let Err(err) = lumina_platform::secure_file::write_private(&path, secret.as_bytes()) {
            tracing::warn!("could not save the phpMyAdmin secret: {err}");
        }
        secret
    }

    /// `bin/.shims`, the one folder Lumina puts on the user's PATH.
    pub fn shim_dir(&self) -> PathBuf {
        self.inner.layout.bin().join(".shims")
    }

    /// What each command in the shim folder runs now: the active PHP (with its
    /// php.ini), Composer on that PHP, the selected Node.js (node, npm, npx),
    /// Python, and the database client already pointed at Lumina's server.
    pub async fn shims(&self, cfg: &AppConfig) -> Vec<crate::tools::Shim> {
        use crate::tools::Shim;
        let bin = self.inner.layout.bin();
        let exe = std::env::consts::EXE_SUFFIX;
        let pick = |tool: &str, wanted: &str| {
            let installs = lumina_packages::installed::list(&bin.join(tool));
            let wanted = wanted.trim();
            installs.into_iter().find(|i| {
                wanted.is_empty()
                    || i.version == wanted
                    || i.version.starts_with(&format!("{wanted}."))
            })
        };
        let mut out = Vec::new();

        let php = self.inner.php.cli(&cfg.php).await;
        let mail = crate::php_fcgi::ini_args(&self.php_mail_ini(cfg).await);
        if let Some((php_exe, ini, version)) = &php {
            let php_args: Vec<String> = ["-c".into(), ini.to_string_lossy().into_owned()]
                .into_iter()
                .chain(mail.iter().cloned())
                .collect();
            out.push(Shim {
                name: "php".into(),
                program: php_exe.clone(),
                args: php_args.clone(),
                describes: format!("PHP {version}"),
            });
            if let Some(composer) = pick("composer", &cfg.tools.composer) {
                let phar = composer.dir.join("composer.phar");
                if phar.is_file() {
                    out.push(Shim {
                        name: "composer".into(),
                        program: php_exe.clone(),
                        args: php_args
                            .iter()
                            .cloned()
                            .chain([phar.to_string_lossy().into_owned()])
                            .collect(),
                        describes: format!("Composer {} on PHP {version}", composer.version),
                    });
                }
            }
        }

        if let Some(node) = pick("node", &cfg.tools.node) {
            let (bin_dir, script) = if cfg!(windows) {
                (node.dir.clone(), ".cmd")
            } else {
                (node.dir.join("bin"), "")
            };
            for (name, file) in [
                ("node", format!("node{exe}")),
                ("npm", format!("npm{script}")),
                ("npx", format!("npx{script}")),
            ] {
                let program = bin_dir.join(file);
                if program.is_file() {
                    out.push(Shim {
                        name: name.into(),
                        program,
                        args: Vec::new(),
                        describes: format!("Node.js {}", node.version),
                    });
                }
            }
        }

        if let Some(python) = pick("python", &cfg.tools.python) {
            let program = [
                python.dir.join(format!("python{exe}")),
                python.dir.join("bin").join(format!("python3{exe}")),
            ]
            .into_iter()
            .find(|p| p.is_file());
            if let Some(program) = program {
                out.push(Shim {
                    name: "python".into(),
                    program,
                    args: Vec::new(),
                    describes: format!("Python {}", python.version),
                });
            }
        }

        let (db_tool, db_wanted) = match cfg.database.engine {
            DatabaseEngine::Mysql => ("mysql", &cfg.database.mysql_version),
            DatabaseEngine::Mariadb => ("mariadb", &cfg.database.mariadb_version),
        };
        if let Some(db) = pick(db_tool, db_wanted) {
            let client = ["mysql", "mariadb"]
                .iter()
                .map(|f| db.dir.join("bin").join(format!("{f}{exe}")))
                .find(|p| p.is_file());
            if let Some(program) = client {
                out.push(Shim {
                    name: "mysql".into(),
                    program,
                    // Defaults for Lumina's server; options given by the user come later and win.
                    args: vec![
                        "--host=127.0.0.1".into(),
                        format!("--port={}", cfg.database.port),
                        "--user=root".into(),
                    ],
                    describes: format!("{db_tool} {} client", db.version),
                });
            }
        }
        out
    }

    /// Rewrites the shim folder for `cfg` (see [`Self::shims`]).
    pub async fn refresh_shims(
        &self,
        cfg: &AppConfig,
        shim_exe: Option<&std::path::Path>,
    ) -> Result<Vec<crate::tools::Shim>> {
        let shims = self.shims(cfg).await;
        crate::tools::write_shims(&self.shim_dir(), shim_exe, &shims)?;
        self.refresh_composer_shims(cfg).await?;
        Ok(shims)
    }

    /// Rewrites the `composer` / `composer.bat` launchers of every installed
    /// Composer so they run with the active PHP and its php.ini. Called after
    /// Composer is installed and whenever the PHP version changes.
    pub async fn refresh_composer_shims(&self, cfg: &AppConfig) -> Result<()> {
        let installs = lumina_packages::installed::list(&self.inner.layout.bin().join("composer"));
        if installs.is_empty() {
            return Ok(());
        }
        let Some((php, ini, version)) = self.inner.php.cli(&cfg.php).await else {
            tracing::info!(
                "no PHP installed yet; Composer launchers are written once there is one"
            );
            return Ok(());
        };
        for install in installs {
            crate::tools::write_composer_shims(&install.dir, &php, &ini, &version)?;
        }
        Ok(())
    }

    /// The local CA and certificates.
    pub fn ssl(&self) -> &Ssl {
        &self.inner.ssl
    }

    /// Why HTTPS is not served although it is on, if it failed.
    pub fn tls_error(&self) -> Option<String> {
        self.inner
            .tls_error
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Certificates for localhost and every site, creating the CA on first use
    /// and renewing what is close to expiry. HTTPS is left out (and the reason
    /// kept for the UI) if that fails, so plain HTTP keeps working.
    fn tls(&self, cfg: &AppConfig, sites: &[lumina_vhost::Site]) -> Option<Tls> {
        let result = (|| -> lumina_ssl::Result<Option<Tls>> {
            if !cfg.general.auto_ssl {
                return Ok(None);
            }
            let (ca, created) = self.inner.ssl.ensure_ca(&host_name(), &cfg.general.tld)?;
            if created {
                tracing::info!(
                    "created the local certificate authority {}",
                    ca.info.subject
                );
            }
            let domains: Vec<String> = sites.iter().map(|s| s.domain.clone()).collect();
            let (localhost, certs) = self.inner.ssl.ensure_all(&ca, &domains)?;
            Ok(Some(Tls {
                https_port: cfg.web.https_port,
                force_https: cfg.general.force_https,
                localhost,
                sites: certs,
            }))
        })();
        let mut error = self
            .inner
            .tls_error
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        match result {
            Ok(tls) => {
                *error = None;
                tls
            }
            Err(err) => {
                tracing::error!("HTTPS is unavailable: {err}");
                *error = Some(err.to_string());
                None
            }
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
                let mail = self.php_mail_ini(cfg).await;
                self.inner.php.start(&cfg.php, &mail).await?;
                self.sync_web_to_php(cfg).await
            }
            ServiceKind::Nginx | ServiceKind::Apache => {
                // The web server needs PHP; start it first if it is down.
                if self.inner.php.status().await.runtime.is_none() {
                    let mail = self.php_mail_ini(cfg).await;
                    self.inner.php.start(&cfg.php, &mail).await?;
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
            ServiceKind::Postgresql => {
                self.inner.postgresql.start(&cfg.postgresql).await?;
                Ok(())
            }
            ServiceKind::Mailpit => {
                self.inner.mailpit.start(&cfg.mailpit).await?;
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
            ServiceKind::Postgresql => self.inner.postgresql.stop().await,
            ServiceKind::Mailpit => self.inner.mailpit.stop().await,
        }
    }

    pub async fn restart(&self, kind: ServiceKind, cfg: &AppConfig) -> Result<()> {
        let _guard = self.inner.lock.lock().await;
        self.restart_locked(kind, cfg).await
    }

    async fn restart_locked(&self, kind: ServiceKind, cfg: &AppConfig) -> Result<()> {
        match kind {
            ServiceKind::PhpFcgi => {
                let mail = self.php_mail_ini(cfg).await;
                self.inner.php.restart(&cfg.php, &mail).await?;
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

    /// What runs right now.
    pub async fn running(&self) -> crate::profiles::Running {
        let database = if self.inner.mysql.status().await.runtime.is_some() {
            Some(ServiceKind::Mysql)
        } else if self.inner.mariadb.status().await.runtime.is_some() {
            Some(ServiceKind::Mariadb)
        } else {
            None
        };
        crate::profiles::Running {
            php: self.inner.php.status().await.runtime.is_some(),
            web: self.running_web().await,
            database,
            redis: self.inner.redis.status().await.runtime.is_some(),
            postgresql: self.inner.postgresql.status().await.runtime.is_some(),
        }
    }

    /// Switches the running stack from `old` to `new` (e.g. after applying a
    /// profile): stops only what changed (web server first, each stop verified
    /// by the supervisor), then starts the replacements (database first; each
    /// start waits for its health check). If anything fails, everything this
    /// switch started is stopped again and what it stopped is started with
    /// `old`, and the error says so.
    pub async fn switch(&self, old: &AppConfig, new: &AppConfig) -> Result<crate::profiles::Plan> {
        let _guard = self.inner.lock.lock().await;
        let running = self.running().await;
        let plan = crate::profiles::plan(&running, old, new);
        if plan.is_empty() {
            return Ok(plan);
        }
        tracing::info!(
            "switching stack: stop {:?}, start {:?}, keep {:?}",
            plan.stop,
            plan.start,
            plan.keep
        );

        let mut stopped = Vec::new();
        let mut started = Vec::new();
        let mut failure = None;
        for &kind in &plan.stop {
            match self.stop_locked(kind).await {
                Ok(()) => stopped.push(kind),
                Err(err) => {
                    failure = Some((kind, err));
                    break;
                }
            }
        }
        if failure.is_none() {
            for &kind in &plan.start {
                if let Err(err) = self.start_locked(kind, new).await {
                    failure = Some((kind, err));
                    break;
                }
                started.push(kind);
            }
        }
        let Some((failed, err)) = failure else {
            // PHP stayed but the web server is new, or the other way round:
            // the web server's config already follows PHP's port.
            return Ok(plan);
        };

        tracing::error!("switch failed at {failed:?}: {err}; restoring the previous stack");
        let mut problems = Vec::new();
        // The failed service may have left a supervised process behind.
        let _ = self.stop_locked(failed).await;
        for &kind in started.iter().rev() {
            if let Err(e) = self.stop_locked(kind).await {
                problems.push(format!("could not stop {kind:?}: {e}"));
            }
        }
        // Restart what was stopped, dependencies first.
        for &kind in stopped.iter().rev() {
            if let Err(e) = self.start_locked(kind, old).await {
                problems.push(format!("could not restart {kind:?}: {e}"));
            }
        }
        let restored = if problems.is_empty() {
            "the previous stack was restored".to_owned()
        } else {
            format!(
                "restoring the previous stack failed too: {}",
                problems.join("; ")
            )
        };
        Err(Error::Service(
            format!("{failed:?}"),
            format!("{err}\n{restored}"),
        ))
    }

    /// Database, then PostgreSQL, Redis and Mailpit (if enabled), PHP, then the configured web server. Every
    /// service is attempted; the first error is returned.
    pub async fn start_all(&self, cfg: &AppConfig) -> Result<()> {
        let _guard = self.inner.lock.lock().await;
        let mut kinds = vec![Self::configured_db(cfg)];
        if cfg.postgresql.enabled {
            kinds.push(ServiceKind::Postgresql);
        }
        if cfg.redis.enabled {
            kinds.push(ServiceKind::Redis);
        }
        if cfg.mailpit.enabled {
            kinds.push(ServiceKind::Mailpit);
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
            ServiceKind::Postgresql => self.inner.postgresql.status().await.runtime.is_some(),
            ServiceKind::Mailpit => self.inner.mailpit.status().await.runtime.is_some(),
        }
    }

    /// A running service's PID and the port it answers on. `None` unless it
    /// runs: stopped, failed, or being restarted by the supervisor.
    async fn probe_target(&self, kind: ServiceKind) -> Option<(u32, u16)> {
        let (status, pid, port) = match kind {
            ServiceKind::PhpFcgi => {
                let s = self.inner.php.status().await;
                (s.status, s.pid, s.runtime.map(|r| r.port))
            }
            ServiceKind::Nginx | ServiceKind::Apache => {
                let s = self.web_status(kind).await;
                (s.status, s.pid, s.runtime.map(|r| r.http_port))
            }
            ServiceKind::Mysql | ServiceKind::Mariadb => {
                let db = if kind == ServiceKind::Mysql {
                    &self.inner.mysql
                } else {
                    &self.inner.mariadb
                };
                let s = db.status().await;
                (s.status, s.pid, s.runtime.map(|r| r.port))
            }
            ServiceKind::Redis => {
                let s = self.inner.redis.status().await;
                (s.status, s.pid, s.runtime.map(|r| r.port))
            }
            ServiceKind::Postgresql => {
                let s = self.inner.postgresql.status().await;
                (s.status, s.pid, s.runtime.map(|r| r.port))
            }
            ServiceKind::Mailpit => {
                let s = self.inner.mailpit.status().await;
                (s.status, s.pid, s.runtime.map(|r| r.smtp_port))
            }
        };
        if status != ServiceStatus::Running {
            return None;
        }
        Some((pid?, port?))
    }

    /// Checks that every running service still accepts connections on its
    /// port, e.g. after the computer woke from sleep, and restarts each one
    /// that does not within `limit`. A crashed service needs no help (the
    /// supervisor restarts it); this catches one that runs but no longer
    /// answers. Returns the services restarted and how each restart went.
    ///
    /// The probes run without the stack lock, so the UI is not held up; a
    /// service stopped or restarted by someone else meanwhile is left alone.
    pub async fn heal(&self, cfg: &AppConfig, limit: Duration) -> Vec<(ServiceKind, Result<()>)> {
        let mut silent = Vec::new();
        // Dependencies first, as in `start_all`.
        for kind in [
            ServiceKind::Mysql,
            ServiceKind::Mariadb,
            ServiceKind::Postgresql,
            ServiceKind::Redis,
            ServiceKind::Mailpit,
            ServiceKind::PhpFcgi,
            ServiceKind::Nginx,
            ServiceKind::Apache,
        ] {
            if let Some((pid, port)) = self.probe_target(kind).await
                && !lumina_process::health::wait_tcp(port, limit).await
            {
                silent.push((kind, pid));
            }
        }

        let mut healed = Vec::new();
        if silent.is_empty() {
            return healed;
        }
        let _guard = self.inner.lock.lock().await;
        for (kind, pid) in silent {
            if self.probe_target(kind).await.map(|(p, _)| p) != Some(pid) {
                continue;
            }
            tracing::warn!("{kind:?} (pid {pid}) no longer answers; restarting it");
            let result = self.restart_locked(kind, cfg).await;
            if let Err(err) = &result {
                tracing::error!("could not restart {kind:?}: {err}");
            }
            healed.push((kind, result));
        }
        healed
    }

    /// Web servers, PHP, Mailpit, Redis, then databases. Returns the services that
    /// failed to stop.
    pub async fn stop_all(&self) -> Vec<(ServiceKind, Error)> {
        let _guard = self.inner.lock.lock().await;
        let mut failures = Vec::new();
        for kind in [
            ServiceKind::Nginx,
            ServiceKind::Apache,
            ServiceKind::PhpFcgi,
            ServiceKind::Mailpit,
            ServiceKind::Redis,
            ServiceKind::Postgresql,
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
            postgresql: self.inner.postgresql.status().await,
            mailpit: self.inner.mailpit.status().await,
        }
    }
}
