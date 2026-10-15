//! PostgreSQL: a server of its own next to MySQL/MariaDB.
//!
//! The first start runs `initdb` into `data/postgresql-<major>/` (superuser
//! `postgres`, no password, UTF-8, listening on 127.0.0.1 only, like MySQL's
//! `root`). Data folders are per major version, because a major upgrade needs
//! `pg_upgrade`; minor versions share one. `postgres` runs under the
//! supervisor and stops with `pg_ctl stop -m fast` (clean shutdown, open
//! connections closed).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use lumina_config::schema::Postgresql as PgConfig;
use lumina_core::paths::Layout;
use lumina_core::types::ServiceStatus;
use lumina_core::{Error, Result};
use lumina_process::{ProcessSpec, ProcessState, RestartPolicy, Supervisor, health, logs};
use serde::Serialize;
use tokio::sync::Mutex;

use crate::install::{self, Install};
use crate::web;

/// Supervisor ID.
pub const PROCESS_ID: &str = "postgresql";
/// The superuser `initdb` creates (no password; local connections only).
pub const SUPERUSER: &str = "postgres";
const READY_TIMEOUT: Duration = Duration::from_secs(60);
const STOP_TIMEOUT: Duration = Duration::from_secs(30);
const TOOL_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PgRuntime {
    pub version: String,
    pub port: u16,
    pub data_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PgStatus {
    pub status: ServiceStatus,
    pub runtime: Option<PgRuntime>,
    pub pid: Option<u32>,
    pub error: Option<String>,
    pub log_file: PathBuf,
}

#[derive(Clone)]
pub struct Postgresql {
    inner: Arc<Inner>,
}

struct Inner {
    supervisor: Supervisor,
    layout: Layout,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    runtime: Option<PgRuntime>,
    install: Option<Install>,
    last_error: Option<String>,
}

impl Postgresql {
    pub fn new(supervisor: Supervisor, layout: Layout) -> Self {
        Self {
            inner: Arc::new(Inner {
                supervisor,
                layout,
                state: Mutex::new(State::default()),
            }),
        }
    }

    fn root(&self) -> PathBuf {
        self.inner.layout.bin().join("postgresql")
    }

    pub fn log_file(&self) -> PathBuf {
        self.inner.layout.logs().join("postgresql.log")
    }

    /// `data/postgresql-<major>/`.
    pub fn data_dir(&self, install: &Install) -> PathBuf {
        self.inner
            .layout
            .data()
            .join(format!("postgresql-{}", major(&install.version)))
    }

    /// Installed versions, newest first.
    pub async fn versions(&self) -> Vec<Install> {
        install::discover(
            &self.root(),
            "postgresql",
            &["bin/postgres"],
            &["--version"],
            parse_version,
        )
        .await
    }

    fn tool(install: &Install, name: &str) -> PathBuf {
        install
            .dir
            .join("bin")
            .join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
    }

    fn initialized(data_dir: &Path) -> bool {
        data_dir.join("PG_VERSION").is_file()
    }

    async fn initialize(&self, install: &Install, data_dir: &Path) -> Result<()> {
        tracing::info!(
            "initialising PostgreSQL {} data directory {}",
            install.version,
            data_dir.display()
        );
        if let Some(parent) = data_dir.parent() {
            std::fs::create_dir_all(parent).map_err(|source| Error::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let pgdata = data_dir.to_string_lossy().into_owned();
        let output = install::run(
            &Self::tool(install, "initdb"),
            &[
                "-D",
                &pgdata,
                "-U",
                SUPERUSER,
                "--auth=trust",
                "--encoding=UTF8",
                "--no-locale",
                "--no-instructions",
            ],
            TOOL_TIMEOUT,
        )
        .await?;
        if output.status.success() && Self::initialized(data_dir) {
            return Ok(());
        }
        let _ = std::fs::remove_dir_all(data_dir);
        let mut detail = String::from_utf8_lossy(&output.stderr).into_owned();
        detail.push_str(&String::from_utf8_lossy(&output.stdout));
        Err(Error::Service(
            PROCESS_ID.into(),
            format!(
                "could not initialise the data directory:\n{}",
                detail.trim()
            ),
        ))
    }

    pub async fn start(&self, cfg: &PgConfig) -> Result<PgRuntime> {
        let mut state = self.inner.state.lock().await;
        let result = self.start_locked(&mut state, cfg).await;
        state.last_error = result.as_ref().err().map(ToString::to_string);
        result
    }

    async fn start_locked(&self, state: &mut State, cfg: &PgConfig) -> Result<PgRuntime> {
        if let Some(info) = self.inner.supervisor.info(PROCESS_ID).await
            && info.state != ProcessState::Failed
        {
            return Err(Error::Service(PROCESS_ID.into(), "already running".into()));
        }
        let installs = self.versions().await;
        let install = install::select(&installs, &cfg.version)
            .ok_or_else(|| {
                install::not_installed(PROCESS_ID, "PostgreSQL", &cfg.version, &self.root())
            })?
            .clone();

        web::check_port(cfg.port)?;
        let data_dir = self.data_dir(&install);
        if !Self::initialized(&data_dir) {
            self.initialize(&install, &data_dir).await?;
        }

        let spec = ProcessSpec {
            id: PROCESS_ID.into(),
            program: Self::tool(&install, "postgres"),
            args: server_args(&data_dir, cfg.port, &self.inner.layout)
                .into_iter()
                .map(Into::into)
                .collect(),
            env: Vec::new(),
            cwd: Some(install.dir.clone()),
            log_file: self.log_file(),
            restart: RestartPolicy::OnFailure {
                max_restarts: 2,
                backoff: Duration::from_secs(1),
            },
        };
        self.inner.supervisor.start(spec).await?;

        if !health::wait_tcp(cfg.port, READY_TIMEOUT).await
            || !Self::wait_accepting(&install, cfg.port).await
        {
            let _ = self.inner.supervisor.stop(PROCESS_ID, STOP_TIMEOUT).await;
            return Err(Error::Service(
                PROCESS_ID.into(),
                format!(
                    "PostgreSQL did not start listening on port {}.\n{}",
                    cfg.port,
                    logs::tail(&self.log_file(), 15)
                ),
            ));
        }

        let runtime = PgRuntime {
            version: install.version.clone(),
            port: cfg.port,
            data_dir,
        };
        tracing::info!(
            "PostgreSQL {} listening on 127.0.0.1:{}",
            install.version,
            cfg.port
        );
        state.runtime = Some(runtime.clone());
        state.install = Some(install);
        Ok(runtime)
    }

    /// The port opens before recovery finishes ("the database system is
    /// starting up"); `pg_isready` exits 0 once connections are accepted.
    async fn wait_accepting(install: &Install, port: u16) -> bool {
        let port = port.to_string();
        let deadline = tokio::time::Instant::now() + READY_TIMEOUT;
        loop {
            let ready = install::run(
                &Self::tool(install, "pg_isready"),
                &["-h", "127.0.0.1", "-p", &port, "-U", SUPERUSER, "-t", "2"],
                Duration::from_secs(5),
            )
            .await
            .is_ok_and(|out| out.status.success());
            if ready {
                return true;
            }
            if tokio::time::Instant::now() >= deadline {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    /// `pg_ctl stop -m fast`: closes connections and checkpoints, then exits.
    pub async fn stop(&self) -> Result<()> {
        let mut state = self.inner.state.lock().await;
        let install = state.install.clone();
        let data_dir = state.runtime.as_ref().map(|r| r.data_dir.clone());
        self.inner
            .supervisor
            .stop_gracefully(PROCESS_ID, STOP_TIMEOUT, async {
                let (Some(install), Some(data_dir)) = (install, data_dir) else {
                    return;
                };
                let pgdata = data_dir.to_string_lossy().into_owned();
                if let Err(err) = install::run(
                    &Self::tool(&install, "pg_ctl"),
                    &["stop", "-D", &pgdata, "-m", "fast", "-w", "-t", "20"],
                    STOP_TIMEOUT,
                )
                .await
                {
                    tracing::warn!("pg_ctl stop failed: {err}");
                }
            })
            .await?;
        state.runtime = None;
        Ok(())
    }

    pub async fn status(&self) -> PgStatus {
        let state = self.inner.state.lock().await;
        let info = self.inner.supervisor.info(PROCESS_ID).await;
        PgStatus {
            status: crate::service_status(info.as_ref(), state.last_error.is_some()),
            runtime: info.as_ref().and(state.runtime.clone()),
            pid: info.as_ref().map(|i| i.pid),
            error: state.last_error.clone(),
            log_file: self.log_file(),
        }
    }
}

/// `postgres` arguments: the data folder, the port, local TCP only. On Unix
/// the socket goes to Lumina's tmp folder (the default /var/run/postgresql may
/// not exist or be writable); Windows has no Unix sockets here.
fn server_args(data_dir: &Path, port: u16, layout: &Layout) -> Vec<String> {
    let mut args = vec![
        "-D".to_owned(),
        data_dir.to_string_lossy().into_owned(),
        "-p".to_owned(),
        port.to_string(),
        "-c".to_owned(),
        "listen_addresses=127.0.0.1".to_owned(),
    ];
    let sockets = if cfg!(windows) {
        String::new()
    } else {
        layout.tmp().to_string_lossy().into_owned()
    };
    args.push("-c".to_owned());
    args.push(format!("unix_socket_directories={sockets}"));
    args
}

/// "18.6" → "18"; "9.6.24" → "9.6" (before 10, the major had two parts).
pub fn major(version: &str) -> String {
    let mut parts = version.split('.');
    let first = parts.next().unwrap_or_default();
    match (first.parse::<u32>(), parts.next()) {
        (Ok(n), Some(second)) if n < 10 => format!("{first}.{second}"),
        _ => first.to_owned(),
    }
}

/// "postgres (PostgreSQL) 18.6" → "18.6".
fn parse_version(text: &str) -> Option<String> {
    let rest = &text[text.find("(PostgreSQL)")? + "(PostgreSQL)".len()..];
    let version: String = rest
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    (!version.is_empty()).then_some(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_version_banner() {
        assert_eq!(
            parse_version("postgres (PostgreSQL) 18.6\n").as_deref(),
            Some("18.6")
        );
        assert_eq!(parse_version("nope"), None);
    }

    #[test]
    fn data_folders_follow_the_major_version() {
        assert_eq!(major("18.6"), "18");
        assert_eq!(major("17.11"), "17");
        assert_eq!(major("9.6.24"), "9.6");
    }

    #[test]
    fn listens_on_localhost_only() {
        let layout = Layout::new(std::env::temp_dir().join("lumina-pg-test"));
        let args = server_args(Path::new("C:/data"), 5433, &layout);
        assert!(args.windows(2).any(|w| w == ["-p", "5433"]));
        assert!(args.contains(&"listen_addresses=127.0.0.1".to_owned()));
        assert!(
            args.iter()
                .any(|a| a.starts_with("unix_socket_directories="))
        );
    }
}
