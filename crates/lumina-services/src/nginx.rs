//! Nginx driver: renders its config into `etc/nginx/`, tests it before every
//! start and reload, reloads with exactly one confirmed signal, and stops
//! gracefully (`nginx -s quit`).
//!
//! Why reloads used to need two tries elsewhere: the signal was sent before the
//! new config was fully written, or with a different `-p` prefix than the
//! running master (so it went to the wrong PID file). Here configs are written
//! atomically and every command uses the same `-p`/`-c`, and the reload is
//! confirmed by watching a new worker appear.

use std::path::PathBuf;
use std::process::Output;
use std::sync::Arc;
use std::time::Duration;

use lumina_config::schema::{Web as WebConfig, WebServer};
use lumina_core::paths::Layout;
use lumina_core::{Error, Result};
use lumina_process::{ProcessSpec, ProcessState, RestartPolicy, Supervisor, health, logs};
use tokio::sync::Mutex;
use tokio::time::{Instant, sleep};

use crate::install::{self, Install};
use crate::template::{ChangeSet, Templates, forward_slashes};
use crate::web::{self, ReloadOutcome, WebContext, WebRuntime, WebStatus};

/// Supervisor ID.
pub const PROCESS_ID: &str = "nginx";
/// Generous: the first start of freshly installed binaries is slowed by antivirus scans.
const READY_TIMEOUT: Duration = Duration::from_secs(30);
const STOP_TIMEOUT: Duration = Duration::from_secs(10);
const COMMAND_TIMEOUT: Duration = Duration::from_secs(15);
/// How long a reload has to show a new worker.
const RELOAD_CONFIRM: Duration = Duration::from_secs(5);
/// How long old workers get to finish and exit after a reload. Until they exit
/// they may still accept connections with the old config (seen on Windows).
const OLD_WORKERS_EXIT: Duration = Duration::from_secs(3);

#[derive(Clone)]
pub struct Nginx {
    inner: Arc<Inner>,
}

struct Inner {
    supervisor: Supervisor,
    layout: Layout,
    templates: Templates,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    runtime: Option<WebRuntime>,
    install: Option<Install>,
    last_error: Option<String>,
}

impl Nginx {
    pub fn new(supervisor: Supervisor, layout: Layout, templates: Templates) -> Self {
        Self {
            inner: Arc::new(Inner {
                supervisor,
                layout,
                templates,
                state: Mutex::new(State::default()),
            }),
        }
    }

    /// `etc/nginx/`, passed as `-p`; holds `logs/` (PID file) and `temp/`.
    pub fn prefix(&self) -> PathBuf {
        self.inner.layout.etc().join("nginx")
    }

    fn conf(&self) -> PathBuf {
        self.prefix().join("nginx.conf")
    }

    /// Server blocks; `00-localhost.conf` and `auto.*.conf` are Lumina's.
    pub fn sites_dir(&self) -> PathBuf {
        self.prefix().join("sites")
    }

    pub fn log_file(&self) -> PathBuf {
        self.inner.layout.logs().join("nginx_error.log")
    }

    fn console_log(&self) -> PathBuf {
        self.inner.layout.logs().join("nginx.log")
    }

    fn root(&self) -> PathBuf {
        self.inner.layout.bin().join("nginx")
    }

    /// Installed versions, newest first.
    pub async fn versions(&self) -> Vec<Install> {
        install::discover(
            &self.root(),
            "nginx",
            &["nginx", "sbin/nginx", "bin/nginx"],
            &["-v"],
            parse_version,
        )
        .await
    }

    /// Renders every config file; returns what changed so it can be undone.
    pub fn prepare(&self, ctx: &WebContext) -> Result<ChangeSet> {
        let prefix = self.prefix();
        for dir in [prefix.join("logs"), prefix.join("temp"), self.sites_dir()] {
            std::fs::create_dir_all(&dir).map_err(|source| Error::Io { path: dir, source })?;
        }
        let t = &self.inner.templates;
        let mut changes = ChangeSet::default();
        t.copy_into(&mut changes, "nginx/mime.types", &prefix.join("mime.types"))?;
        t.copy_into(
            &mut changes,
            "nginx/fastcgi_params",
            &prefix.join("fastcgi_params"),
        )?;

        let mut c = tera::Context::new();
        c.insert("windows", &cfg!(windows));
        c.insert("logs_dir", &forward_slashes(&self.inner.layout.logs()));
        c.insert("sites_dir", &forward_slashes(&self.sites_dir()));
        c.insert("php_port", &ctx.php_port);
        c.insert("http_port", &ctx.http_port);
        c.insert("www_root", &forward_slashes(&ctx.www_root));
        web::insert_tls(&mut c, ctx);
        web::render_db_ui(t, &mut changes, &mut c, ctx)?;
        t.render_into(
            &mut changes,
            "nginx/fastcgi.conf.tera",
            &c,
            &prefix.join("fastcgi.conf"),
        )?;
        t.render_into(&mut changes, "nginx/nginx.conf.tera", &c, &self.conf())?;
        t.render_into(
            &mut changes,
            "nginx/localhost.conf.tera",
            &c,
            &self.sites_dir().join("00-localhost.conf"),
        )?;
        web::render_sites(
            t,
            &mut changes,
            "nginx/site.conf.tera",
            &self.sites_dir(),
            ctx,
        )?;
        Ok(changes)
    }

    /// Runs nginx with the prefix and config of this data root plus `extra`.
    async fn command(&self, install: &Install, extra: &[&str]) -> Result<Output> {
        // nginx wants the prefix with a trailing slash.
        let prefix = format!("{}/", forward_slashes(&self.prefix()));
        let conf = forward_slashes(&self.conf());
        let mut args = vec!["-p", &prefix, "-c", &conf];
        args.extend_from_slice(extra);
        install::run(&install.exe, &args, COMMAND_TIMEOUT).await
    }

    /// `nginx -t`; the error carries nginx's own explanation.
    pub async fn config_test(&self, install: &Install) -> Result<()> {
        let out = self.command(install, &["-t"]).await?;
        if out.status.success() {
            return Ok(());
        }
        Err(Error::Service(
            PROCESS_ID.into(),
            format!(
                "configuration test failed:\n{}",
                String::from_utf8_lossy(&out.stderr).trim()
            ),
        ))
    }

    pub async fn start(&self, cfg: &WebConfig, ctx: &WebContext) -> Result<WebRuntime> {
        let mut state = self.inner.state.lock().await;
        let result = self.start_locked(&mut state, cfg, ctx).await;
        state.last_error = result.as_ref().err().map(ToString::to_string);
        result
    }

    async fn start_locked(
        &self,
        state: &mut State,
        cfg: &WebConfig,
        ctx: &WebContext,
    ) -> Result<WebRuntime> {
        if let Some(info) = self.inner.supervisor.info(PROCESS_ID).await
            && info.state != ProcessState::Failed
        {
            return Err(Error::Service(PROCESS_ID.into(), "already running".into()));
        }
        let installs = self.versions().await;
        let install = install::select(&installs, &cfg.nginx_version)
            .ok_or_else(|| {
                install::not_installed(PROCESS_ID, "nginx", &cfg.nginx_version, &self.root())
            })?
            .clone();

        web::check_ports(ctx, None)?;
        self.ensure_landing_page(ctx)?;
        let changes = self.prepare(ctx)?;
        if let Err(err) = self.config_test(&install).await {
            changes.rollback();
            return Err(err);
        }

        let prefix = format!("{}/", forward_slashes(&self.prefix()));
        let spec = ProcessSpec {
            id: PROCESS_ID.into(),
            program: install.exe.clone(),
            args: vec![
                "-p".into(),
                prefix.into(),
                "-c".into(),
                forward_slashes(&self.conf()).into(),
            ],
            env: Vec::new(),
            cwd: Some(install.dir.clone()),
            log_file: self.console_log(),
            restart: RestartPolicy::OnFailure {
                max_restarts: 3,
                backoff: Duration::from_secs(1),
            },
        };
        self.inner.supervisor.start(spec).await?;

        if !health::wait_tcp(ctx.http_port, READY_TIMEOUT).await {
            let _ = self.inner.supervisor.stop(PROCESS_ID, STOP_TIMEOUT).await;
            return Err(Error::Service(
                PROCESS_ID.into(),
                format!(
                    "nginx did not start listening on port {}.\n{}",
                    ctx.http_port,
                    logs::tail(&self.log_file(), 15)
                ),
            ));
        }

        let runtime = WebRuntime {
            server: WebServer::Nginx,
            version: install.version.clone(),
            http_port: ctx.http_port,
            https_port: ctx.https_port(),
            php_port: ctx.php_port,
        };
        tracing::info!(
            "nginx {} listening on 127.0.0.1:{}",
            install.version,
            ctx.http_port
        );
        state.runtime = Some(runtime.clone());
        state.install = Some(install);
        Ok(runtime)
    }

    /// Graceful stop (`nginx -s quit`: finish in-flight requests), then verified.
    pub async fn stop(&self) -> Result<()> {
        let mut state = self.inner.state.lock().await;
        let install = state.install.clone();
        self.inner
            .supervisor
            .stop_gracefully(PROCESS_ID, STOP_TIMEOUT, async {
                if let Some(install) = &install
                    && let Err(err) = self.command(install, &["-s", "quit"]).await
                {
                    tracing::warn!("nginx -s quit failed: {err}");
                }
            })
            .await?;
        state.runtime = None;
        Ok(())
    }

    /// Re-renders the config and, if it changed, applies it to the running
    /// server with a single confirmed reload. A config that fails `nginx -t` is
    /// rolled back and nothing is signalled.
    pub async fn reload(&self, ctx: &WebContext) -> Result<ReloadOutcome> {
        let mut state = self.inner.state.lock().await;
        let (Some(runtime), Some(install)) = (state.runtime.clone(), state.install.clone()) else {
            return Err(Error::Service(PROCESS_ID.into(), "not running".into()));
        };
        web::check_ports(ctx, Some(&runtime))?;

        let changes = self.prepare(ctx)?;
        if !changes.changed() {
            return Ok(ReloadOutcome::Unchanged);
        }
        if let Err(err) = self.config_test(&install).await {
            changes.rollback();
            return Err(err);
        }

        let mut signals = 0;
        let before = self.workers().await;
        loop {
            signals += 1;
            let out = self.command(&install, &["-s", "reload"]).await?;
            if !out.status.success() {
                return Err(Error::Service(
                    PROCESS_ID.into(),
                    format!(
                        "reload failed: {}",
                        String::from_utf8_lossy(&out.stderr).trim()
                    ),
                ));
            }
            if self.wait_new_worker(&before).await {
                self.wait_old_workers_gone(&before).await;
                break;
            }
            if signals == 2 {
                return Err(Error::Service(
                    PROCESS_ID.into(),
                    "reload was not confirmed: no new worker appeared".into(),
                ));
            }
            tracing::warn!("nginx reload not confirmed within {RELOAD_CONFIRM:?}; retrying once");
        }

        tracing::info!("nginx reloaded ({signals} signal(s))");
        state.runtime = Some(WebRuntime {
            http_port: ctx.http_port,
            https_port: ctx.https_port(),
            php_port: ctx.php_port,
            ..runtime
        });
        Ok(ReloadOutcome::Reloaded { signals })
    }

    /// Worker PIDs: nginx processes in the group other than the master.
    async fn workers(&self) -> Vec<u32> {
        let master = self.inner.supervisor.info(PROCESS_ID).await.map(|i| i.pid);
        let members = self.inner.supervisor.members(PROCESS_ID).await;
        web::named(&members, "nginx")
            .into_iter()
            .filter(|pid| Some(*pid) != master)
            .collect()
    }

    /// The master starts a fresh worker for every reload.
    async fn wait_new_worker(&self, before: &[u32]) -> bool {
        let deadline = Instant::now() + RELOAD_CONFIRM;
        while Instant::now() < deadline {
            if self.workers().await.iter().any(|w| !before.contains(w)) {
                return true;
            }
            sleep(Duration::from_millis(50)).await;
        }
        false
    }

    /// After a confirmed reload, waits briefly for the old workers to exit so
    /// the new config is what answers the next request. Old workers that are
    /// still busy with long requests are left to finish (no extra signal).
    async fn wait_old_workers_gone(&self, before: &[u32]) {
        let deadline = Instant::now() + OLD_WORKERS_EXIT;
        while Instant::now() < deadline {
            let current = self.workers().await;
            if !before.iter().any(|old| current.contains(old)) {
                return;
            }
            sleep(Duration::from_millis(25)).await;
        }
        tracing::info!("old nginx workers are still finishing requests after the reload");
    }

    pub async fn status(&self) -> WebStatus {
        let state = self.inner.state.lock().await;
        let info = self.inner.supervisor.info(PROCESS_ID).await;
        let members = self.inner.supervisor.members(PROCESS_ID).await;
        WebStatus {
            server: WebServer::Nginx,
            status: crate::service_status(info.as_ref(), state.last_error.is_some()),
            runtime: info.as_ref().and(state.runtime.clone()),
            pid: info.as_ref().map(|i| i.pid),
            processes: web::named(&members, "nginx").len(),
            error: state.last_error.clone(),
            log_file: self.log_file(),
        }
    }

    fn ensure_landing_page(&self, ctx: &WebContext) -> Result<()> {
        let page = self.inner.templates.dir().join("../www/index.html");
        web::ensure_landing_page(&ctx.www_root, &page)
    }
}

/// "nginx version: nginx/1.30.5" → "1.30.5".
fn parse_version(text: &str) -> Option<String> {
    let rest = &text[text.find("nginx/")? + "nginx/".len()..];
    let version: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    (!version.is_empty()).then_some(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_version_banner() {
        assert_eq!(
            parse_version("nginx version: nginx/1.30.5\n").as_deref(),
            Some("1.30.5")
        );
        assert_eq!(parse_version("nope"), None);
    }

    #[test]
    fn renders_https_and_the_force_https_redirect() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let layout = Layout::new(tmp.path().join("Lumina"));
        std::fs::create_dir_all(layout.www().join("blog")).expect("mkdir");
        let templates = Templates::new(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/templates"),
        );
        let nginx = Nginx::new(
            Supervisor::new(lumina_process::ledger::Ledger::disabled()),
            layout.clone(),
            templates,
        );
        let ssl = lumina_ssl::Ssl::new(layout.ssl());
        let (ca, _) = ssl.ensure_ca("PC", "test").expect("ca");
        let (localhost, certs) = ssl.ensure_all(&ca, &["blog.test".into()]).expect("certs");
        let cfg = lumina_config::AppConfig::default();
        let mut ctx = WebContext {
            http_port: 80,
            php_port: 10987,
            www_root: layout.www(),
            sites: lumina_vhost::scan(&layout.www(), &cfg).sites,
            tls: Some(lumina_ssl::Tls {
                https_port: 8443,
                force_https: false,
                localhost,
                sites: certs,
            }),
            db_ui: None,
        };
        nginx.prepare(&ctx).expect("prepare");

        let site =
            std::fs::read_to_string(nginx.sites_dir().join("auto.blog.test.conf")).expect("site");
        assert!(site.contains("listen       127.0.0.1:8443 ssl;"), "{site}");
        assert!(site.contains("blog.test.crt\""), "{site}");
        assert!(!site.contains("return  301"), "{site}");
        let local = std::fs::read_to_string(nginx.sites_dir().join("00-localhost.conf"))
            .expect("localhost");
        assert!(
            local.contains("listen       127.0.0.1:8443 ssl default_server;"),
            "{local}"
        );
        let conf = std::fs::read_to_string(nginx.conf()).expect("nginx.conf");
        assert!(conf.contains("ssl_protocols"), "{conf}");

        if let Some(tls) = ctx.tls.as_mut() {
            tls.force_https = true;
        }
        assert!(nginx.prepare(&ctx).expect("force").changed());
        let site =
            std::fs::read_to_string(nginx.sites_dir().join("auto.blog.test.conf")).expect("site");
        assert!(
            site.contains("return  301 https://$host:8443$request_uri;"),
            "{site}"
        );

        // HTTPS off → plain HTTP only.
        ctx.tls = None;
        nginx.prepare(&ctx).expect("off");
        let site =
            std::fs::read_to_string(nginx.sites_dir().join("auto.blog.test.conf")).expect("site");
        assert!(!site.contains("ssl"), "{site}");
    }

    #[test]
    fn renders_config_with_ports_and_php_pool() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let layout = Layout::new(tmp.path().join("Lumina"));
        let templates = Templates::new(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/templates"),
        );
        let nginx = Nginx::new(
            Supervisor::new(lumina_process::ledger::Ledger::disabled()),
            layout.clone(),
            templates,
        );
        let blog = layout.www().join("blog");
        std::fs::create_dir_all(blog.join("public")).expect("mkdir");
        let mut cfg = lumina_config::AppConfig::default();
        let mut ctx = WebContext {
            http_port: 8080,
            php_port: 20987,
            www_root: layout.www(),
            sites: lumina_vhost::scan(&layout.www(), &cfg).sites,
            tls: None,
            db_ui: None,
        };

        let changes = nginx.prepare(&ctx).expect("prepare");
        assert!(changes.changed());

        let conf = std::fs::read_to_string(nginx.conf()).expect("nginx.conf");
        assert!(
            conf.contains("server  127.0.0.1:20987 max_fails=0;"),
            "{conf}"
        );
        let site =
            std::fs::read_to_string(nginx.sites_dir().join("00-localhost.conf")).expect("site");
        assert!(
            site.contains("listen       127.0.0.1:8080 default_server;"),
            "{site}"
        );
        let root = site
            .lines()
            .find(|l| l.trim_start().starts_with("root"))
            .expect("root line");
        assert!(!root.contains('\\'), "paths use forward slashes: {root}");
        let fastcgi =
            std::fs::read_to_string(nginx.prefix().join("fastcgi.conf")).expect("fastcgi");
        assert!(fastcgi.contains("fastcgi_next_upstream "));
        assert!(
            !fastcgi.lines().any(|l| l.starts_with("fastcgi_keep_conn")),
            "one PHP connection per request"
        );

        // Same input → nothing to write → no reload needed.
        assert!(!nginx.prepare(&ctx).expect("again").changed());

        // The automatic site for www/blog serves blog/public.
        let auto = nginx.sites_dir().join("auto.blog.test.conf");
        let site = std::fs::read_to_string(&auto).expect("auto site");
        assert!(
            site.contains("server_name  blog.test *.blog.test;"),
            "{site}"
        );
        assert!(site.contains("listen       127.0.0.1:8080;"), "{site}");
        assert!(
            site.contains(&format!(
                "root         \"{}\";",
                forward_slashes(&blog.join("public"))
            )),
            "{site}"
        );

        // A user file for the domain replaces the automatic one.
        let user = nginx.sites_dir().join("blog.test.conf");
        std::fs::write(&user, "# mine").expect("user file");
        assert!(nginx.prepare(&ctx).expect("user override").changed());
        assert!(!auto.exists());
        std::fs::remove_file(&user).expect("rm");

        // Removing the folder (or turning auto vhosts off) removes the site;
        // a rollback brings it back.
        nginx.prepare(&ctx).expect("auto again");
        cfg.general.auto_vhosts = false;
        ctx.sites = lumina_vhost::scan(&layout.www(), &cfg).sites;
        let changes = nginx.prepare(&ctx).expect("no sites");
        assert!(!auto.exists());
        changes.rollback();
        assert!(auto.exists());
        assert!(
            nginx.sites_dir().join("00-localhost.conf").exists(),
            "the localhost site is never removed"
        );
    }
}
