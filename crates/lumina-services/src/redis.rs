//! Redis: renders `etc/redis/redis.conf`, keeps data in `data/redis/`, and stops
//! with `redis-cli shutdown` (which saves a snapshot first).
//!
//! Windows builds of Redis are compiled with Cygwin or MSYS2. Those runtimes
//! read `C:/...` as a relative path, so paths are passed in their
//! `/cygdrive/c/...` form when the build ships such a runtime.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use lumina_config::schema::Redis as RedisConfig;
use lumina_core::paths::Layout;
use lumina_core::types::ServiceStatus;
use lumina_core::{Error, Result};
use lumina_process::{ProcessSpec, ProcessState, RestartPolicy, Supervisor, health, logs};
use serde::Serialize;
use tokio::sync::Mutex;

use crate::install::{self, Install};
use crate::template::{Templates, write_if_changed};
use crate::web;

/// Supervisor ID.
pub const PROCESS_ID: &str = "redis";
const READY_TIMEOUT: Duration = Duration::from_secs(30);
const STOP_TIMEOUT: Duration = Duration::from_secs(30);
const CLI_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RedisRuntime {
    pub version: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RedisStatus {
    pub status: ServiceStatus,
    pub runtime: Option<RedisRuntime>,
    pub pid: Option<u32>,
    pub error: Option<String>,
    pub log_file: PathBuf,
}

#[derive(Clone)]
pub struct Redis {
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
    runtime: Option<RedisRuntime>,
    install: Option<Install>,
    last_error: Option<String>,
}

impl Redis {
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

    fn root(&self) -> PathBuf {
        self.inner.layout.bin().join("redis")
    }

    fn conf(&self) -> PathBuf {
        self.inner.layout.etc().join("redis").join("redis.conf")
    }

    pub fn log_file(&self) -> PathBuf {
        self.inner.layout.logs().join("redis.log")
    }

    /// Installed versions, newest first.
    pub async fn versions(&self) -> Vec<Install> {
        install::discover(
            &self.root(),
            &["redis-server", "bin/redis-server", "src/redis-server"],
            &["--version"],
            parse_version,
        )
        .await
    }

    /// Renders `redis.conf` (and the empty `custom.conf` it includes).
    pub fn render(&self, install: &Install, port: u16) -> Result<PathBuf> {
        let conf = self.conf();
        let custom = conf.with_file_name("custom.conf");
        if !custom.exists() {
            write_if_changed(
                &custom,
                b"# Your own Redis settings. Lumina never edits this file.\n",
            )?;
        }
        let data = self.inner.layout.data().join("redis");
        std::fs::create_dir_all(&data).map_err(|source| Error::Io {
            path: data.clone(),
            source,
        })?;

        let mut c = tera::Context::new();
        c.insert("version", &install.version);
        c.insert("port", &port);
        c.insert("data_dir", &redis_path(install, &data));
        c.insert("log_file", &redis_path(install, &self.log_file()));
        c.insert("custom_conf", &redis_path(install, &custom));
        self.inner
            .templates
            .render_to("redis/redis.conf.tera", &c, &conf)?;
        Ok(conf)
    }

    pub async fn start(&self, cfg: &RedisConfig) -> Result<RedisRuntime> {
        let mut state = self.inner.state.lock().await;
        let result = self.start_locked(&mut state, cfg).await;
        state.last_error = result.as_ref().err().map(ToString::to_string);
        result
    }

    async fn start_locked(&self, state: &mut State, cfg: &RedisConfig) -> Result<RedisRuntime> {
        if let Some(info) = self.inner.supervisor.info(PROCESS_ID).await
            && info.state != ProcessState::Failed
        {
            return Err(Error::Service(PROCESS_ID.into(), "already running".into()));
        }
        let installs = self.versions().await;
        let install = install::select(&installs, &cfg.version)
            .ok_or_else(|| install::not_installed(PROCESS_ID, "Redis", &cfg.version, &self.root()))?
            .clone();

        web::check_port(cfg.port)?;
        let conf = self.render(&install, cfg.port)?;
        let spec = ProcessSpec {
            id: PROCESS_ID.into(),
            program: install.exe.clone(),
            args: vec![redis_path(&install, &conf).into()],
            env: Vec::new(),
            cwd: Some(install.dir.clone()),
            // Redis writes to its own logfile; this captures start-up errors.
            log_file: self.inner.layout.logs().join("redis-console.log"),
            restart: RestartPolicy::OnFailure {
                max_restarts: 2,
                backoff: Duration::from_secs(1),
            },
        };
        self.inner.supervisor.start(spec).await?;

        if !health::wait_tcp(cfg.port, READY_TIMEOUT).await {
            let _ = self.inner.supervisor.stop(PROCESS_ID, STOP_TIMEOUT).await;
            let console = logs::tail(&self.inner.layout.logs().join("redis-console.log"), 10);
            return Err(Error::Service(
                PROCESS_ID.into(),
                format!(
                    "Redis did not start listening on port {}.\n{console}\n{}",
                    cfg.port,
                    logs::tail(&self.log_file(), 10)
                ),
            ));
        }

        let runtime = RedisRuntime {
            version: install.version.clone(),
            port: cfg.port,
        };
        tracing::info!(
            "Redis {} listening on 127.0.0.1:{}",
            install.version,
            cfg.port
        );
        state.runtime = Some(runtime.clone());
        state.install = Some(install);
        Ok(runtime)
    }

    /// `redis-cli shutdown`: saves a snapshot, then exits.
    pub async fn stop(&self) -> Result<()> {
        let mut state = self.inner.state.lock().await;
        let (install, port) = (
            state.install.clone(),
            state.runtime.as_ref().map(|r| r.port),
        );
        self.inner
            .supervisor
            .stop_gracefully(PROCESS_ID, STOP_TIMEOUT, async {
                let (Some(install), Some(port)) = (install, port) else {
                    return;
                };
                let Some(cli) = install
                    .exe
                    .parent()
                    .map(|d| d.join(format!("redis-cli{}", std::env::consts::EXE_SUFFIX)))
                else {
                    return;
                };
                let port = port.to_string();
                if let Err(err) = install::run(&cli, &["-p", &port, "shutdown"], CLI_TIMEOUT).await
                {
                    tracing::warn!("redis-cli shutdown failed: {err}");
                }
            })
            .await?;
        state.runtime = None;
        Ok(())
    }

    pub async fn status(&self) -> RedisStatus {
        let state = self.inner.state.lock().await;
        let info = self.inner.supervisor.info(PROCESS_ID).await;
        RedisStatus {
            status: crate::service_status(info.as_ref(), state.last_error.is_some()),
            runtime: info.as_ref().and(state.runtime.clone()),
            pid: info.as_ref().map(|i| i.pid),
            error: state.last_error.clone(),
            log_file: self.log_file(),
        }
    }
}

/// A path as this Redis build expects it: `/cygdrive/c/...` for Cygwin/MSYS2
/// builds on Windows, unchanged otherwise.
fn redis_path(install: &Install, path: &Path) -> String {
    let posix_runtime = ["msys-2.0.dll", "cygwin1.dll"]
        .iter()
        .any(|dll| install.dir.join(dll).is_file());
    if cfg!(windows) && posix_runtime {
        to_cygdrive(path)
    } else {
        path.to_string_lossy().into_owned()
    }
}

/// `C:\Lumina\etc\redis.conf` → `/cygdrive/c/Lumina/etc/redis.conf`.
fn to_cygdrive(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    let text = text.strip_prefix("//?/").unwrap_or(&text);
    match text.as_bytes() {
        [drive, b':', rest @ ..] if drive.is_ascii_alphabetic() => format!(
            "/cygdrive/{}{}",
            (*drive as char).to_ascii_lowercase(),
            String::from_utf8_lossy(rest)
        ),
        _ => text.to_owned(),
    }
}

/// "Redis server v=8.10.2 sha=..." → "8.10.2".
fn parse_version(text: &str) -> Option<String> {
    let rest = &text[text.find("v=")? + 2..];
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
    fn converts_windows_paths_for_cygwin_builds() {
        assert_eq!(
            to_cygdrive(Path::new(r"C:\Lumina\etc\redis.conf")),
            "/cygdrive/c/Lumina/etc/redis.conf"
        );
        assert_eq!(to_cygdrive(Path::new(r"\\?\D:\data")), "/cygdrive/d/data");
        assert_eq!(to_cygdrive(Path::new("/home/a/x")), "/home/a/x");
    }

    #[test]
    fn parses_version_banner() {
        assert_eq!(
            parse_version("Redis server v=8.10.2 sha=00000000:0 malloc=libc bits=64").as_deref(),
            Some("8.10.2")
        );
    }
}
