//! PHP FastCGI: spawns `php-cgi -b 127.0.0.1:<port>` with PHP_FCGI_CHILDREN /
//! PHP_FCGI_MAX_REQUESTS, enforces the child minimum, and fully terminates the
//! old process tree on stop and on PHP version change (the supervisor verifies
//! the group is empty before a new version starts).

pub mod env;
pub mod versions;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use lumina_config::schema::Php as PhpConfig;
use lumina_core::constants::MIN_PHP_FCGI_CHILDREN;
use lumina_core::paths::Layout;
use lumina_core::types::ServiceStatus;
use lumina_core::{Error, Result};
use lumina_process::{ProcessSpec, ProcessState, RestartPolicy, Supervisor, health, logs, ports};
use serde::Serialize;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::template::Templates;

pub use versions::PhpInstall;

/// Supervisor ID of the PHP pool.
pub const PROCESS_ID: &str = "php-fcgi";
/// How long php-cgi gets to start listening.
/// Generous: the first start of freshly installed binaries is slowed by antivirus scans.
const READY_TIMEOUT: Duration = Duration::from_secs(30);
const STOP_TIMEOUT: Duration = Duration::from_secs(10);
/// How often the watchdog counts php-cgi processes.
const WATCHDOG_INTERVAL: Duration = Duration::from_secs(30);

/// What is running right now.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhpRuntime {
    pub version: String,
    pub port: u16,
    /// `true` when the configured port was busy and the fallback port is used.
    pub fell_back: bool,
    /// Effective PHP_FCGI_CHILDREN (after enforcing the minimum).
    pub children: u32,
    pub max_requests: u32,
    pub ini: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhpStatus {
    pub status: ServiceStatus,
    pub runtime: Option<PhpRuntime>,
    pub pid: Option<u32>,
    /// Live php-cgi processes (the master plus its workers).
    pub processes: usize,
    pub restarts: u32,
    /// Why the last start failed, if it did.
    pub error: Option<String>,
    pub log_file: PathBuf,
}

/// The PHP FastCGI service. Cheap to clone.
#[derive(Clone)]
pub struct PhpFcgi {
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
    runtime: Option<PhpRuntime>,
    spec: Option<ProcessSpec>,
    watchdog: Option<JoinHandle<()>>,
    last_error: Option<String>,
}

impl PhpFcgi {
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

    pub fn log_file(&self) -> PathBuf {
        self.inner.layout.logs().join("php-fcgi.log")
    }

    fn php_root(&self) -> PathBuf {
        self.inner.layout.bin().join("php")
    }

    /// Installed PHP versions, newest first.
    pub async fn versions(&self) -> Vec<PhpInstall> {
        versions::discover(&self.php_root()).await
    }

    /// The PHP command-line binary and php.ini for `cfg.version` (creating the
    /// ini if needed), for tools such as Composer. `None` without PHP.
    pub async fn cli(&self, cfg: &PhpConfig) -> Option<(PathBuf, PathBuf, String)> {
        let installs = self.versions().await;
        let install = versions::select(&installs, &cfg.version)?;
        let exe = ["php", "bin/php"]
            .iter()
            .map(|rel| {
                install
                    .dir
                    .join(format!("{rel}{}", std::env::consts::EXE_SUFFIX))
            })
            .find(|p| p.is_file())?;
        let ini = env::ensure_ini(&self.inner.layout, &self.inner.templates, install).ok()?;
        Some((exe, ini, install.version.clone()))
    }

    /// The PHP version that serves requests: the running one, else the one
    /// `cfg` selects.
    pub async fn active_version(&self, cfg: &PhpConfig) -> Option<String> {
        if let Some(rt) = self.status().await.runtime {
            return Some(rt.version);
        }
        let installs = self.versions().await;
        versions::select(&installs, &cfg.version).map(|i| i.version.clone())
    }

    /// Starts the pool with `cfg`. Fails if it is already running.
    pub async fn start(&self, cfg: &PhpConfig) -> Result<PhpRuntime> {
        let mut state = self.inner.state.lock().await;
        let result = self.start_locked(&mut state, cfg).await;
        state.last_error = result.as_ref().err().map(ToString::to_string);
        result
    }

    async fn start_locked(&self, state: &mut State, cfg: &PhpConfig) -> Result<PhpRuntime> {
        if let Some(info) = self.inner.supervisor.info(PROCESS_ID).await
            && info.state != ProcessState::Failed
        {
            return Err(Error::Service(PROCESS_ID.into(), "already running".into()));
        }

        let installs = self.versions().await;
        let install = versions::select(&installs, &cfg.version).ok_or_else(|| {
            let wanted = if cfg.version.is_empty() {
                "any version".to_owned()
            } else {
                format!("PHP {}", cfg.version)
            };
            Error::Service(
                PROCESS_ID.into(),
                format!("{wanted} is not installed in {}", self.php_root().display()),
            )
        })?;

        let ini = env::ensure_ini(&self.inner.layout, &self.inner.templates, install)?;
        let ini_dir = env::ini_dir(&self.inner.layout, install);
        let choice = ports::pick(cfg.fcgi_port, cfg.fcgi_fallback_port)?;
        let children = env::effective_children(cfg.fcgi_children);

        let spec = ProcessSpec {
            id: PROCESS_ID.into(),
            program: install.exe.clone(),
            args: vec!["-b".into(), format!("127.0.0.1:{}", choice.port).into()],
            env: env::fcgi_env(install, &ini_dir, children, cfg.fcgi_max_requests),
            cwd: Some(install.dir.clone()),
            log_file: self.log_file(),
            restart: RestartPolicy::OnFailure {
                max_restarts: 3,
                backoff: Duration::from_secs(1),
            },
        };
        self.inner.supervisor.start(spec.clone()).await?;

        if !health::wait_tcp(choice.port, READY_TIMEOUT).await {
            let _ = self.inner.supervisor.stop(PROCESS_ID, STOP_TIMEOUT).await;
            let tail = logs::tail(&self.log_file(), 15);
            return Err(Error::Service(
                PROCESS_ID.into(),
                format!(
                    "php-cgi did not start listening on port {} within {}s.\n{tail}",
                    choice.port,
                    READY_TIMEOUT.as_secs()
                ),
            ));
        }

        let runtime = PhpRuntime {
            version: install.version.clone(),
            port: choice.port,
            fell_back: choice.fell_back,
            children,
            max_requests: cfg.fcgi_max_requests,
            ini,
        };
        tracing::info!(
            "PHP {} listening on 127.0.0.1:{} (children {children})",
            runtime.version,
            runtime.port
        );
        if let Some(old) = state.watchdog.take() {
            old.abort();
        }
        state.watchdog = Some(tokio::spawn(watchdog(self.clone())));
        state.runtime = Some(runtime.clone());
        state.spec = Some(spec);
        Ok(runtime)
    }

    /// Stops the pool; succeeds only once every php-cgi process is gone.
    pub async fn stop(&self) -> Result<()> {
        let mut state = self.inner.state.lock().await;
        self.stop_locked(&mut state).await
    }

    async fn stop_locked(&self, state: &mut State) -> Result<()> {
        if let Some(watchdog) = state.watchdog.take() {
            watchdog.abort();
        }
        self.inner.supervisor.stop(PROCESS_ID, STOP_TIMEOUT).await?;
        state.runtime = None;
        state.spec = None;
        Ok(())
    }

    /// Stop, verify, start: used for restarts and PHP version switches.
    pub async fn restart(&self, cfg: &PhpConfig) -> Result<PhpRuntime> {
        let mut state = self.inner.state.lock().await;
        self.stop_locked(&mut state).await?;
        let result = self.start_locked(&mut state, cfg).await;
        state.last_error = result.as_ref().err().map(ToString::to_string);
        result
    }

    pub async fn status(&self) -> PhpStatus {
        let state = self.inner.state.lock().await;
        let info = self.inner.supervisor.info(PROCESS_ID).await;
        let processes = self.php_cgi_processes().await.len();
        let status = crate::service_status(info.as_ref(), state.last_error.is_some());
        PhpStatus {
            status,
            runtime: info.as_ref().and(state.runtime.clone()),
            pid: info.as_ref().map(|i| i.pid),
            processes,
            restarts: info.as_ref().map_or(0, |i| i.restarts),
            error: state.last_error.clone(),
            log_file: self.log_file(),
        }
    }

    /// php-cgi processes in the pool's group (ignores helpers such as conhost.exe).
    async fn php_cgi_processes(&self) -> Vec<u32> {
        let members = self.inner.supervisor.members(PROCESS_ID).await;
        crate::web::named(&members, "php-cgi")
    }
}

/// Restarts the pool when fewer php-cgi processes are alive than the minimum
/// (master + MIN_PHP_FCGI_CHILDREN workers), e.g. after workers crashed and the
/// master failed to replace them.
async fn watchdog(php: PhpFcgi) {
    loop {
        tokio::time::sleep(WATCHDOG_INTERVAL).await;
        let alive = php.php_cgi_processes().await.len();
        let minimum = MIN_PHP_FCGI_CHILDREN as usize + 1;
        if alive >= minimum {
            continue;
        }
        let Some(info) = php.inner.supervisor.info(PROCESS_ID).await else {
            return;
        };
        if info.state != ProcessState::Running {
            continue; // the supervisor is already restarting it
        }
        tracing::warn!("PHP process count too low ({alive} < {minimum}); restarting the PHP pool");
        let mut state = php.inner.state.lock().await;
        let Some(spec) = state.spec.clone() else {
            return;
        };
        let result = async {
            php.inner.supervisor.stop(PROCESS_ID, STOP_TIMEOUT).await?;
            php.inner.supervisor.start(spec).await
        }
        .await;
        if let Err(err) = result {
            tracing::error!("watchdog restart failed: {err}");
            state.last_error = Some(err.to_string());
            state.runtime = None;
            return;
        }
    }
}
