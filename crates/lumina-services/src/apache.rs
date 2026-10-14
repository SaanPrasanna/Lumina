//! Apache httpd driver: renders `etc/apache/httpd.conf`, tests it before every
//! start and reload, and hands PHP to the FastCGI pool via `mod_proxy_fcgi`.
//!
//! Reload: on Unix `httpd -k graceful`. On Windows a console-mode httpd (not a
//! service) has no graceful reload, so it is stopped and started; that takes a
//! moment because PHP runs separately and is not restarted.

use std::path::PathBuf;
use std::process::Output;
use std::sync::Arc;
use std::time::Duration;

use lumina_config::schema::{Web as WebConfig, WebServer};
use lumina_core::paths::Layout;
use lumina_core::{Error, Result};
use lumina_process::{ProcessSpec, ProcessState, RestartPolicy, Supervisor, health, logs};
use tokio::sync::Mutex;

use crate::install::{self, Install};
use crate::template::{ChangeSet, Templates, forward_slashes};
use crate::web::{self, ReloadOutcome, WebContext, WebRuntime, WebStatus};

/// Supervisor ID.
pub const PROCESS_ID: &str = "apache";
/// Generous: the first start of freshly installed binaries is slowed by antivirus scans.
const READY_TIMEOUT: Duration = Duration::from_secs(30);
const STOP_TIMEOUT: Duration = Duration::from_secs(10);
const COMMAND_TIMEOUT: Duration = Duration::from_secs(15);

/// Modules loaded when the build ships them (`modules/mod_<name>.so`).
const MODULES: &[&str] = &[
    "unixd",
    "authz_core",
    "authz_host",
    "access_compat",
    "alias",
    "autoindex",
    "dir",
    "env",
    "expires",
    "filter",
    "deflate",
    "headers",
    "log_config",
    "mime",
    "rewrite",
    "setenvif",
    "proxy",
    "proxy_fcgi",
    "socache_shmcb",
    "ssl",
];

#[derive(Clone)]
pub struct Apache {
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

impl Apache {
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

    /// `etc/apache/`.
    pub fn conf_dir(&self) -> PathBuf {
        self.inner.layout.etc().join("apache")
    }

    fn conf(&self) -> PathBuf {
        self.conf_dir().join("httpd.conf")
    }

    /// Virtual hosts; `00-localhost.conf` and `auto.*.conf` are Lumina's.
    pub fn sites_dir(&self) -> PathBuf {
        self.conf_dir().join("sites")
    }

    pub fn log_file(&self) -> PathBuf {
        self.inner.layout.logs().join("apache_error.log")
    }

    fn root(&self) -> PathBuf {
        self.inner.layout.bin().join("apache")
    }

    /// Installed versions, newest first.
    pub async fn versions(&self) -> Vec<Install> {
        install::discover(
            &self.root(),
            &["bin/httpd", "sbin/httpd"],
            &["-v"],
            parse_version,
        )
        .await
    }

    /// Renders httpd.conf and the localhost site for `install`.
    pub fn prepare(&self, install: &Install, ctx: &WebContext) -> Result<ChangeSet> {
        let sites = self.sites_dir();
        std::fs::create_dir_all(&sites).map_err(|source| Error::Io {
            path: sites.clone(),
            source,
        })?;
        let modules_dir = install.dir.join("modules");
        let modules: Vec<&str> = MODULES
            .iter()
            .copied()
            .filter(|m| modules_dir.join(format!("mod_{m}.so")).is_file())
            .collect();
        if ctx.tls.is_some() && !modules.contains(&"ssl") {
            return Err(Error::Service(
                PROCESS_ID.into(),
                format!(
                    "this Apache build has no mod_ssl ({}), so HTTPS cannot be served; turn HTTPS off or use nginx",
                    modules_dir.join("mod_ssl.so").display()
                ),
            ));
        }

        let mut c = tera::Context::new();
        c.insert("windows", &cfg!(windows));
        c.insert("server_root", &forward_slashes(&install.dir));
        c.insert("modules", &modules);
        c.insert("logs_dir", &forward_slashes(&self.inner.layout.logs()));
        c.insert("sites_dir", &forward_slashes(&sites));
        c.insert(
            "pid_file",
            &forward_slashes(&self.conf_dir().join("httpd.pid")),
        );
        c.insert("php_port", &ctx.php_port);
        c.insert("http_port", &ctx.http_port);
        c.insert("www_root", &forward_slashes(&ctx.www_root));
        web::insert_tls(&mut c, ctx);

        let t = &self.inner.templates;
        let mut changes = ChangeSet::default();
        t.render_into(&mut changes, "apache/httpd.conf.tera", &c, &self.conf())?;
        t.render_into(
            &mut changes,
            "apache/localhost.conf.tera",
            &c,
            &sites.join("00-localhost.conf"),
        )?;
        web::render_sites(t, &mut changes, "apache/site.conf.tera", &sites, ctx)?;
        Ok(changes)
    }

    fn base_args(&self, install: &Install) -> Vec<String> {
        vec![
            "-d".into(),
            forward_slashes(&install.dir),
            "-f".into(),
            forward_slashes(&self.conf()),
        ]
    }

    async fn command(&self, install: &Install, extra: &[&str]) -> Result<Output> {
        let base = self.base_args(install);
        let mut args: Vec<&str> = base.iter().map(String::as_str).collect();
        args.extend_from_slice(extra);
        install::run(&install.exe, &args, COMMAND_TIMEOUT).await
    }

    /// `httpd -t`; the error carries Apache's own explanation.
    pub async fn config_test(&self, install: &Install) -> Result<()> {
        let out = self.command(install, &["-t"]).await?;
        if out.status.success() {
            return Ok(());
        }
        let mut text = String::from_utf8_lossy(&out.stderr).trim().to_owned();
        if text.is_empty() {
            text = String::from_utf8_lossy(&out.stdout).trim().to_owned();
        }
        Err(Error::Service(
            PROCESS_ID.into(),
            format!("configuration test failed:\n{text}"),
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
        let install = install::select(&installs, &cfg.apache_version)
            .ok_or_else(|| {
                install::not_installed(PROCESS_ID, "Apache", &cfg.apache_version, &self.root())
            })?
            .clone();

        web::check_ports(ctx, None)?;
        let page = self.inner.templates.dir().join("../www/index.html");
        web::ensure_landing_page(&ctx.www_root, &page)?;
        let changes = self.prepare(&install, ctx)?;
        if let Err(err) = self.config_test(&install).await {
            changes.rollback();
            return Err(err);
        }

        let mut args: Vec<std::ffi::OsString> = self
            .base_args(&install)
            .into_iter()
            .map(Into::into)
            .collect();
        if cfg!(unix) {
            args.push("-DFOREGROUND".into());
        }
        let spec = ProcessSpec {
            id: PROCESS_ID.into(),
            program: install.exe.clone(),
            args,
            env: Vec::new(),
            cwd: Some(install.dir.clone()),
            log_file: self.inner.layout.logs().join("apache.log"),
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
                    "Apache did not start listening on port {}.\n{}",
                    ctx.http_port,
                    logs::tail(&self.log_file(), 15)
                ),
            ));
        }

        let runtime = WebRuntime {
            server: WebServer::Apache,
            version: install.version.clone(),
            http_port: ctx.http_port,
            https_port: ctx.https_port(),
            php_port: ctx.php_port,
        };
        tracing::info!(
            "Apache {} listening on 127.0.0.1:{}",
            install.version,
            ctx.http_port
        );
        state.runtime = Some(runtime.clone());
        state.install = Some(install);
        Ok(runtime)
    }

    pub async fn stop(&self) -> Result<()> {
        let mut state = self.inner.state.lock().await;
        self.inner.supervisor.stop(PROCESS_ID, STOP_TIMEOUT).await?;
        state.runtime = None;
        Ok(())
    }

    /// Applies a changed config: graceful reload on Unix, restart on Windows.
    /// A config that fails `httpd -t` is rolled back first.
    pub async fn reload(&self, cfg: &WebConfig, ctx: &WebContext) -> Result<ReloadOutcome> {
        let mut state = self.inner.state.lock().await;
        let (Some(runtime), Some(install)) = (state.runtime.clone(), state.install.clone()) else {
            return Err(Error::Service(PROCESS_ID.into(), "not running".into()));
        };
        let changes = self.prepare(&install, ctx)?;
        if !changes.changed() {
            return Ok(ReloadOutcome::Unchanged);
        }
        if let Err(err) = self.config_test(&install).await {
            changes.rollback();
            return Err(err);
        }

        let ports_changed =
            ctx.http_port != runtime.http_port || ctx.https_port() != runtime.https_port;
        if ports_changed {
            web::check_ports(ctx, Some(&runtime))?;
        }
        if cfg!(windows) || ports_changed {
            self.inner.supervisor.stop(PROCESS_ID, STOP_TIMEOUT).await?;
            state.runtime = None;
            let result = self.start_locked(&mut state, cfg, ctx).await;
            state.last_error = result.as_ref().err().map(ToString::to_string);
            result?;
            return Ok(ReloadOutcome::Restarted);
        }

        let out = self.command(&install, &["-k", "graceful"]).await?;
        if !out.status.success() {
            return Err(Error::Service(
                PROCESS_ID.into(),
                format!(
                    "graceful reload failed: {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                ),
            ));
        }
        state.runtime = Some(WebRuntime {
            php_port: ctx.php_port,
            ..runtime
        });
        Ok(ReloadOutcome::Reloaded { signals: 1 })
    }

    pub async fn status(&self) -> WebStatus {
        let state = self.inner.state.lock().await;
        let info = self.inner.supervisor.info(PROCESS_ID).await;
        let members = self.inner.supervisor.members(PROCESS_ID).await;
        WebStatus {
            server: WebServer::Apache,
            status: crate::service_status(info.as_ref(), state.last_error.is_some()),
            runtime: info.as_ref().and(state.runtime.clone()),
            pid: info.as_ref().map(|i| i.pid),
            processes: web::named(&members, "httpd").len(),
            error: state.last_error.clone(),
            log_file: self.log_file(),
        }
    }
}

/// "Server version: Apache/2.4.69 (Win64)" → "2.4.69".
fn parse_version(text: &str) -> Option<String> {
    let rest = &text[text.find("Apache/")? + "Apache/".len()..];
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
        let banner = "Server version: Apache/2.4.69 (Win64)\nApache Lounge VS18 Server built: ...";
        assert_eq!(parse_version(banner).as_deref(), Some("2.4.69"));
    }

    #[test]
    fn renders_only_available_modules() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let layout = Layout::new(tmp.path().join("Lumina"));
        let install_dir = tmp.path().join("apache");
        std::fs::create_dir_all(install_dir.join("modules")).expect("mkdir");
        for m in ["mime", "proxy", "proxy_fcgi"] {
            std::fs::write(install_dir.join("modules").join(format!("mod_{m}.so")), b"")
                .expect("w");
        }
        let install = Install {
            version: "2.4.69".into(),
            dir: install_dir,
            exe: PathBuf::from("httpd"),
        };
        let templates = Templates::new(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/templates"),
        );
        let apache = Apache::new(
            Supervisor::new(lumina_process::ledger::Ledger::disabled()),
            layout.clone(),
            templates,
        );
        std::fs::create_dir_all(layout.www().join("Shop")).expect("mkdir");
        let ctx = WebContext {
            http_port: 80,
            php_port: 10987,
            www_root: layout.www(),
            sites: lumina_vhost::scan(&layout.www(), &lumina_config::AppConfig::default()).sites,
            tls: None,
        };

        apache.prepare(&install, &ctx).expect("prepare");

        let conf = std::fs::read_to_string(apache.conf()).expect("httpd.conf");
        assert!(conf.contains("LoadModule proxy_fcgi_module modules/mod_proxy_fcgi.so"));
        assert!(!conf.contains("rewrite_module"), "not shipped → not loaded");
        assert!(conf.contains("fcgi://127.0.0.1:10987/"));
        assert!(conf.contains("Listen 127.0.0.1:80"));

        let site = std::fs::read_to_string(apache.sites_dir().join("auto.shop.test.conf"))
            .expect("auto site");
        assert!(site.contains("<VirtualHost 127.0.0.1:80>"), "{site}");
        assert!(site.contains("ServerName shop.test"), "{site}");
        assert!(
            site.contains(&format!(
                "DocumentRoot \"{}\"",
                forward_slashes(&layout.www().join("Shop"))
            )),
            "{site}"
        );
    }
}
