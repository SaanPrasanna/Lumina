//! MySQL / MariaDB: creates the data directory on first start (root@127.0.0.1
//! with an empty password, local only), renders `etc/<engine>/my.cnf`, and stops
//! with the engine's own `shutdown` so InnoDB never needs crash recovery.
//!
//! Data lives in `data/<engine>-<major.minor>/`: patch updates reuse it, while a
//! new release line starts fresh and leaves the old data untouched.

pub mod engine;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use lumina_config::schema::{Database as DbConfig, DatabaseEngine};
use lumina_core::paths::Layout;
use lumina_core::types::ServiceStatus;
use lumina_core::{Error, Result};
use lumina_process::{ProcessSpec, ProcessState, RestartPolicy, Supervisor, health, logs};
use serde::Serialize;
use sysinfo::System;
use tokio::sync::Mutex;

pub use engine::Engine;

use crate::install::{self, Install};
use crate::template::{Templates, forward_slashes, write_if_changed};
use crate::web;

const READY_TIMEOUT: Duration = Duration::from_secs(60);
/// InnoDB flushes on shutdown; give it time before killing.
const STOP_TIMEOUT: Duration = Duration::from_secs(60);
const INIT_TIMEOUT: Duration = Duration::from_secs(180);
const ADMIN_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DbRuntime {
    pub engine: DatabaseEngine,
    pub version: String,
    pub port: u16,
    pub data_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DbStatus {
    pub engine: DatabaseEngine,
    pub status: ServiceStatus,
    pub runtime: Option<DbRuntime>,
    pub pid: Option<u32>,
    pub error: Option<String>,
    /// The server's error log.
    pub log_file: PathBuf,
}

/// One database engine. The orchestrator holds one per engine and runs at most
/// one of them (they share the port).
#[derive(Clone)]
pub struct Database {
    inner: Arc<Inner>,
}

struct Inner {
    engine: Engine,
    supervisor: Supervisor,
    layout: Layout,
    templates: Templates,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    runtime: Option<DbRuntime>,
    install: Option<Install>,
    last_error: Option<String>,
}

impl Database {
    pub fn new(
        engine: Engine,
        supervisor: Supervisor,
        layout: Layout,
        templates: Templates,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                engine,
                supervisor,
                layout,
                templates,
                state: Mutex::new(State::default()),
            }),
        }
    }

    pub fn engine(&self) -> Engine {
        self.inner.engine
    }

    fn id(&self) -> &'static str {
        self.inner.engine.id
    }

    fn root(&self) -> PathBuf {
        self.inner.layout.bin().join(self.id())
    }

    fn conf_dir(&self) -> PathBuf {
        self.inner.layout.etc().join(self.id())
    }

    fn cnf(&self) -> PathBuf {
        self.conf_dir().join("my.cnf")
    }

    pub fn log_file(&self) -> PathBuf {
        self.inner
            .layout
            .logs()
            .join(format!("{}_error.log", self.id()))
    }

    /// `data/<engine>-<major.minor>/`.
    pub fn data_dir(&self, install: &Install) -> PathBuf {
        self.inner.layout.data().join(format!(
            "{}-{}",
            self.id(),
            engine::release_line(&install.version)
        ))
    }

    /// Unix socket shared with PHP's `mysqli.default_socket`; none on Windows.
    pub fn socket(layout: &Layout) -> Option<PathBuf> {
        cfg!(unix).then(|| layout.tmp().join("mysql.sock"))
    }

    fn configured_version<'a>(&self, cfg: &'a DbConfig) -> &'a str {
        match self.inner.engine.kind {
            DatabaseEngine::Mysql => &cfg.mysql_version,
            DatabaseEngine::Mariadb => &cfg.mariadb_version,
        }
    }

    /// Installed versions, newest first.
    pub async fn versions(&self) -> Vec<Install> {
        install::discover(
            &self.root(),
            // The Homebrew formulas are named like the engines.
            self.inner.engine.id,
            self.inner.engine.server,
            &["--version"],
            engine::parse_version,
        )
        .await
    }

    /// Renders `my.cnf` (and creates the empty `custom.cnf` it includes).
    pub fn render(&self, install: &Install, port: u16) -> Result<PathBuf> {
        let conf_dir = self.conf_dir();
        let custom = conf_dir.join("custom.cnf");
        if !custom.exists() {
            write_if_changed(
                &custom,
                b"# Your own [mysqld] settings. Lumina never edits this file.\n",
            )?;
        }
        let total_mb = {
            let mut sys = System::new();
            sys.refresh_memory();
            sys.total_memory() / (1024 * 1024)
        };

        let mut c = tera::Context::new();
        c.insert("engine_name", self.inner.engine.name);
        c.insert("mysql", &(self.inner.engine.kind == DatabaseEngine::Mysql));
        c.insert("version", &install.version);
        c.insert("windows", &cfg!(windows));
        c.insert("basedir", &forward_slashes(&install.dir));
        c.insert("datadir", &forward_slashes(&self.data_dir(install)));
        c.insert("port", &port);
        c.insert(
            "socket",
            &Self::socket(&self.inner.layout).map(|s| forward_slashes(&s)),
        );
        c.insert("tmpdir", &forward_slashes(&self.inner.layout.tmp()));
        c.insert("error_log", &forward_slashes(&self.log_file()));
        c.insert(
            "pid_file",
            &forward_slashes(&conf_dir.join(format!("{}.pid", self.id()))),
        );
        c.insert("buffer_pool_mb", &buffer_pool_mb(total_mb));
        c.insert("low_memory", &(total_mb < 8 * 1024));
        c.insert("custom_cnf", &forward_slashes(&custom));

        let cnf = self.cnf();
        self.inner
            .templates
            .render_to("mysql/my.cnf.tera", &c, &cnf)?;
        Ok(cnf)
    }

    /// A data directory exists once it has the `mysql` system schema.
    fn initialized(data_dir: &Path) -> bool {
        data_dir.join("mysql").is_dir()
    }

    /// Creates a fresh data directory with root@127.0.0.1 / root@::1 and no
    /// password. On failure the half-created directory is removed.
    async fn initialize(
        &self,
        install: &Install,
        cnf: &Path,
        data_dir: &Path,
        port: u16,
    ) -> Result<()> {
        tracing::info!(
            "initialising {} data directory {}",
            self.inner.engine.name,
            data_dir.display()
        );
        if let Some(parent) = data_dir.parent() {
            std::fs::create_dir_all(parent).map_err(|source| Error::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let output = match self.inner.engine.kind {
            DatabaseEngine::Mysql => {
                let sql = self.conf_dir().join("init-root.sql");
                self.inner.templates.copy_to("mysql/init-root.sql", &sql)?;
                install::run(
                    &install.exe,
                    &[
                        // --defaults-file must come first.
                        &format!("--defaults-file={}", forward_slashes(cnf)),
                        "--initialize-insecure",
                        &format!("--init-file={}", forward_slashes(&sql)),
                    ],
                    INIT_TIMEOUT,
                )
                .await?
            }
            DatabaseEngine::Mariadb => {
                let tool = find_tool(
                    &install.dir,
                    &[
                        "bin/mariadb-install-db",
                        "bin/mysql_install_db",
                        "scripts/mariadb-install-db",
                    ],
                )
                .ok_or_else(|| {
                    Error::Service(self.id().into(), "mariadb-install-db not found".into())
                })?;
                let datadir = format!("--datadir={}", forward_slashes(data_dir));
                let port_arg = format!("--port={port}");
                let basedir = format!("--basedir={}", forward_slashes(&install.dir));
                let args: Vec<&str> = if cfg!(windows) {
                    // Creates root@localhost, root@127.0.0.1 and root@::1 without a password.
                    vec![&datadir, &port_arg]
                } else {
                    vec![
                        &basedir,
                        &datadir,
                        "--auth-root-authentication-method=normal",
                        "--skip-test-db",
                    ]
                };
                install::run(&tool, &args, INIT_TIMEOUT).await?
            }
        };

        if output.status.success() && Self::initialized(data_dir) {
            return Ok(());
        }
        let _ = std::fs::remove_dir_all(data_dir);
        let mut detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        if detail.is_empty() {
            detail = logs::tail(&self.log_file(), 15);
        }
        Err(Error::Service(
            self.id().into(),
            format!("could not initialise the data directory:\n{detail}"),
        ))
    }

    pub async fn start(&self, cfg: &DbConfig) -> Result<DbRuntime> {
        let mut state = self.inner.state.lock().await;
        let result = self.start_locked(&mut state, cfg).await;
        state.last_error = result.as_ref().err().map(ToString::to_string);
        result
    }

    async fn start_locked(&self, state: &mut State, cfg: &DbConfig) -> Result<DbRuntime> {
        let id = self.id();
        if let Some(info) = self.inner.supervisor.info(id).await
            && info.state != ProcessState::Failed
        {
            return Err(Error::Service(id.into(), "already running".into()));
        }
        let installs = self.versions().await;
        let wanted = self.configured_version(cfg);
        let install = install::select(&installs, wanted)
            .ok_or_else(|| {
                install::not_installed(id, self.inner.engine.name, wanted, &self.root())
            })?
            .clone();

        web::check_port(cfg.port)?;
        let data_dir = self.data_dir(&install);
        let cnf = self.render(&install, cfg.port)?;
        if !Self::initialized(&data_dir) {
            self.initialize(&install, &cnf, &data_dir, cfg.port).await?;
        }

        let spec = ProcessSpec {
            id: id.into(),
            program: install.exe.clone(),
            args: vec![format!("--defaults-file={}", forward_slashes(&cnf)).into()],
            env: Vec::new(),
            cwd: Some(install.dir.clone()),
            log_file: self.inner.layout.logs().join(format!("{id}.log")),
            restart: RestartPolicy::OnFailure {
                max_restarts: 2,
                backoff: Duration::from_secs(2),
            },
        };
        self.inner.supervisor.start(spec).await?;

        if !health::wait_tcp(cfg.port, READY_TIMEOUT).await {
            let _ = self.inner.supervisor.stop(id, STOP_TIMEOUT).await;
            return Err(Error::Service(
                id.into(),
                format!(
                    "{} did not start listening on port {}.\n{}",
                    self.inner.engine.name,
                    cfg.port,
                    logs::tail(&self.log_file(), 15)
                ),
            ));
        }

        let runtime = DbRuntime {
            engine: self.inner.engine.kind,
            version: install.version.clone(),
            port: cfg.port,
            data_dir,
        };
        tracing::info!(
            "{} {} listening on 127.0.0.1:{}",
            self.inner.engine.name,
            install.version,
            cfg.port
        );
        state.runtime = Some(runtime.clone());
        state.install = Some(install);
        Ok(runtime)
    }

    /// `mysqladmin shutdown` so InnoDB flushes and closes cleanly; killed only if
    /// that does not finish in time (e.g. the root password was changed).
    pub async fn stop(&self) -> Result<()> {
        let mut state = self.inner.state.lock().await;
        let (install, port) = (
            state.install.clone(),
            state.runtime.as_ref().map(|r| r.port),
        );
        self.inner
            .supervisor
            .stop_gracefully(self.id(), STOP_TIMEOUT, async {
                let (Some(install), Some(port)) = (install, port) else {
                    return;
                };
                let Some(admin) = find_tool(&install.dir, self.inner.engine.admin) else {
                    return;
                };
                let port = port.to_string();
                let result = install::run(
                    &admin,
                    &["-h", "127.0.0.1", "-P", &port, "-u", "root", "shutdown"],
                    ADMIN_TIMEOUT,
                )
                .await;
                match result {
                    Ok(out) if out.status.success() => {}
                    Ok(out) => tracing::warn!(
                        "graceful {} shutdown failed; it will be terminated: {}",
                        self.inner.engine.name,
                        String::from_utf8_lossy(&out.stderr).trim()
                    ),
                    Err(err) => tracing::warn!("graceful shutdown failed: {err}"),
                }
            })
            .await?;
        state.runtime = None;
        Ok(())
    }

    pub async fn status(&self) -> DbStatus {
        let state = self.inner.state.lock().await;
        let info = self.inner.supervisor.info(self.id()).await;
        DbStatus {
            engine: self.inner.engine.kind,
            status: crate::service_status(info.as_ref(), state.last_error.is_some()),
            runtime: info.as_ref().and(state.runtime.clone()),
            pid: info.as_ref().map(|i| i.pid),
            error: state.last_error.clone(),
            log_file: self.log_file(),
        }
    }
}

/// First existing executable among `candidates` (relative, without `.exe`).
fn find_tool(dir: &Path, candidates: &[&str]) -> Option<PathBuf> {
    candidates
        .iter()
        .map(|rel| dir.join(format!("{rel}{}", std::env::consts::EXE_SUFFIX)))
        .find(|p| p.is_file())
}

/// 12.5 % of RAM, at least 128 MB, rounded down to InnoDB's 128 MB chunks.
fn buffer_pool_mb(total_mb: u64) -> u64 {
    ((total_mb / 8) / 128 * 128).max(128)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_pool_is_an_eighth_of_ram_in_chunks() {
        assert_eq!(buffer_pool_mb(16 * 1024), 2048);
        assert_eq!(buffer_pool_mb(4 * 1024), 512);
        assert_eq!(buffer_pool_mb(512), 128, "never below 128 MB");
        assert_eq!(buffer_pool_mb(10_000), 1152, "rounded to 128 MB chunks");
    }

    #[test]
    fn renders_mysql_and_mariadb_specific_options() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let layout = Layout::new(tmp.path().join("Lumina"));
        let templates = Templates::new(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/templates"),
        );
        let install = Install {
            version: "8.4.11".into(),
            dir: tmp.path().join("mysql"),
            exe: tmp.path().join("mysqld"),
            homebrew: None,
        };
        let sup = Supervisor::new(lumina_process::ledger::Ledger::disabled());

        let mysql = Database::new(
            engine::MYSQL,
            sup.clone(),
            layout.clone(),
            templates.clone(),
        );
        let cnf =
            std::fs::read_to_string(mysql.render(&install, 3307).expect("render")).expect("read");
        assert!(cnf.contains("port = 3307"), "{cnf}");
        assert!(cnf.contains("mysqlx = OFF"));
        assert!(cnf.contains("skip-name-resolve"));
        assert!(cnf.contains("data/mysql-8.4\""), "per release line: {cnf}");
        assert!(layout.etc().join("mysql/custom.cnf").exists());

        let maria = Database::new(engine::MARIADB, sup, layout.clone(), templates);
        let cnf =
            std::fs::read_to_string(maria.render(&install, 3306).expect("render")).expect("read");
        assert!(!cnf.contains("mysqlx"), "MariaDB rejects mysqlx");
    }
}
