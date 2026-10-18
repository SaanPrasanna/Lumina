//! More services from Quick Add, described as data rather than one module
//! each: MongoDB, Valkey, Meilisearch, Typesense and SeaweedFS (S3). Each is
//! one program with a port, a data folder under `data/<id>/` and a log in
//! `logs/<id>.log`; Lumina starts it like the built-in services and stops it
//! with Stop All.
//!
//! Settings live in `[services.<id>]` (`enabled`, `version`, `port`); a missing
//! section means "off, newest version, default port".

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use lumina_config::schema::ExtraService as ExtraConfig;
use lumina_core::paths::Layout;
use lumina_core::types::ServiceStatus;
use lumina_core::{Error, Result};
use lumina_process::{ProcessSpec, ProcessState, RestartPolicy, Supervisor, health, logs};
use serde::Serialize;
use tokio::sync::Mutex;

use crate::install::{self, Install};
use crate::web;

const READY_TIMEOUT: Duration = Duration::from_secs(60);
const STOP_TIMEOUT: Duration = Duration::from_secs(20);

/// The values a service's command line is made from.
pub struct Launch<'a> {
    pub port: u16,
    pub data: &'a Path,
}

/// One service Lumina can run.
pub struct Definition {
    /// Package ID, `bin/<id>/` folder and supervisor ID.
    pub id: &'static str,
    pub name: &'static str,
    /// Program inside an install, without `.exe`; the first that exists is used.
    pub exe: &'static [&'static str],
    pub version_args: &'static [&'static str],
    /// Homebrew formula found on macOS/Linux, like the built-in servers.
    pub homebrew: &'static str,
    pub default_port: u16,
    /// What the UI says about connecting, `{port}` replaced.
    pub connect: &'static str,
    /// A web page to open, `{port}` replaced (a dashboard or console).
    pub web_ui: Option<&'static str>,
    pub args: fn(&Launch<'_>) -> Vec<OsString>,
}

fn os(args: &[String]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}

fn path(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

pub const DEFINITIONS: &[Definition] = &[
    Definition {
        id: "mongodb",
        name: "MongoDB",
        exe: &["bin/mongod", "mongod"],
        version_args: &["--version"],
        homebrew: "mongodb-community",
        default_port: 27017,
        connect: "mongodb://127.0.0.1:{port}",
        web_ui: None,
        args: |l| {
            os(&[
                "--dbpath".into(),
                path(l.data),
                "--port".into(),
                l.port.to_string(),
                "--bind_ip".into(),
                "127.0.0.1".into(),
            ])
        },
    },
    Definition {
        id: "valkey",
        name: "Valkey",
        exe: &["bin/valkey-server", "valkey-server", "src/valkey-server"],
        version_args: &["--version"],
        homebrew: "valkey",
        // Next to Redis, which keeps 6379.
        default_port: 6380,
        connect: "redis://127.0.0.1:{port}",
        web_ui: None,
        args: |l| {
            os(&[
                "--port".into(),
                l.port.to_string(),
                "--bind".into(),
                "127.0.0.1".into(),
                "--dir".into(),
                path(l.data),
                "--protected-mode".into(),
                "no".into(),
                "--save".into(),
                "60 1".into(),
            ])
        },
    },
    Definition {
        id: "meilisearch",
        name: "Meilisearch",
        exe: &["meilisearch"],
        version_args: &["--version"],
        homebrew: "meilisearch",
        default_port: 7700,
        connect: "http://127.0.0.1:{port} (development mode: no master key)",
        web_ui: Some("http://127.0.0.1:{port}/"),
        args: |l| {
            os(&[
                "--http-addr".into(),
                format!("127.0.0.1:{}", l.port),
                "--db-path".into(),
                path(&l.data.join("data.ms")),
                "--dump-dir".into(),
                path(&l.data.join("dumps")),
                "--snapshot-dir".into(),
                path(&l.data.join("snapshots")),
                "--env".into(),
                "development".into(),
                "--no-analytics".into(),
            ])
        },
    },
    Definition {
        id: "typesense",
        name: "Typesense",
        exe: &["typesense-server"],
        version_args: &["--version"],
        homebrew: "typesense-server",
        default_port: 8108,
        connect: "http://127.0.0.1:{port}, API key: lumina",
        web_ui: None,
        args: |l| {
            os(&[
                format!("--data-dir={}", path(l.data)),
                "--api-key=lumina".into(),
                "--listen-address=127.0.0.1".into(),
                format!("--api-port={}", l.port),
                "--peering-address=127.0.0.1".into(),
                format!("--peering-port={}", l.port - 1),
                "--enable-cors".into(),
            ])
        },
    },
    Definition {
        id: "seaweedfs",
        name: "SeaweedFS (S3)",
        exe: &["weed"],
        version_args: &["version"],
        homebrew: "seaweedfs",
        default_port: 8333,
        connect: "S3 endpoint http://127.0.0.1:{port}, any access key; path-style URLs",
        web_ui: None,
        // `weed mini`: master, volume, filer and S3 in one process. Every port
        // follows the S3 port so nothing lands on busy defaults such as 8080.
        args: |l| {
            let p = l.port;
            os(&[
                "mini".into(),
                format!("-dir={}", path(l.data)),
                "-ip=127.0.0.1".into(),
                "-ip.bind=127.0.0.1".into(),
                format!("-s3.port={p}"),
                format!("-master.port={}", p + 1),
                format!("-volume.port={}", p + 2),
                format!("-filer.port={}", p + 3),
                format!("-admin.port={}", p + 4),
                format!("-webdav.port={}", p + 5),
            ])
        },
    },
];

pub fn definition(id: &str) -> Option<&'static Definition> {
    DEFINITIONS.iter().find(|d| d.id == id)
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ExtraRuntime {
    pub version: String,
    pub port: u16,
}

/// One service for the Services page.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ExtraStatus {
    pub id: String,
    pub name: String,
    pub status: ServiceStatus,
    pub runtime: Option<ExtraRuntime>,
    pub pid: Option<u32>,
    pub error: Option<String>,
    pub log_file: PathBuf,
    /// Installed versions, newest first.
    pub versions: Vec<String>,
    /// How apps connect, for the configured port.
    pub connect: String,
    /// A web page the service serves (dashboard), for the configured port.
    pub web_ui: Option<String>,
    /// Settings with defaults filled in.
    pub enabled: bool,
    pub version: String,
    pub port: u16,
}

/// The settings for `id`, with defaults for a missing section or port 0.
pub fn settings(services: &BTreeMap<String, ExtraConfig>, def: &Definition) -> ExtraConfig {
    let mut cfg = services.get(def.id).cloned().unwrap_or_default();
    if cfg.port == 0 {
        cfg.port = def.default_port;
    }
    cfg
}

#[derive(Clone)]
pub struct Extras {
    inner: Arc<Inner>,
}

struct Inner {
    supervisor: Supervisor,
    layout: Layout,
    /// Per service: what runs, and why the last start failed.
    state: Mutex<BTreeMap<&'static str, State>>,
}

#[derive(Default, Clone)]
struct State {
    runtime: Option<ExtraRuntime>,
    last_error: Option<String>,
}

impl Extras {
    pub fn new(supervisor: Supervisor, layout: Layout) -> Self {
        Self {
            inner: Arc::new(Inner {
                supervisor,
                layout,
                state: Mutex::new(BTreeMap::new()),
            }),
        }
    }

    pub fn log_file(&self, def: &Definition) -> PathBuf {
        self.inner.layout.logs().join(format!("{}.log", def.id))
    }

    pub async fn versions(&self, def: &Definition) -> Vec<Install> {
        install::discover(
            &self.inner.layout.bin().join(def.id),
            def.homebrew,
            def.exe,
            def.version_args,
            parse_version,
        )
        .await
    }

    pub async fn start(&self, def: &'static Definition, cfg: &ExtraConfig) -> Result<()> {
        let result = self.start_inner(def, cfg).await;
        let mut states = self.inner.state.lock().await;
        let state = states.entry(def.id).or_default();
        match result {
            Ok(runtime) => {
                state.runtime = Some(runtime);
                state.last_error = None;
                Ok(())
            }
            Err(err) => {
                let result = Err(err);
                state.last_error = crate::start_failure(&result);
                result
            }
        }
    }

    async fn start_inner(
        &self,
        def: &'static Definition,
        cfg: &ExtraConfig,
    ) -> Result<ExtraRuntime> {
        if let Some(info) = self.inner.supervisor.info(def.id).await
            && info.state != ProcessState::Failed
        {
            return Err(Error::Service(def.id.into(), "already running".into()));
        }
        let root = self.inner.layout.bin().join(def.id);
        let install = install::select(&self.versions(def).await, &cfg.version)
            .cloned()
            .ok_or_else(|| install::not_installed(def.id, def.name, &cfg.version, &root))?;
        web::check_port(cfg.port)?;
        let data = self.inner.layout.data().join(def.id);
        std::fs::create_dir_all(&data).map_err(|source| Error::Io {
            path: data.clone(),
            source,
        })?;
        let spec = ProcessSpec {
            id: def.id.into(),
            program: install.exe.clone(),
            args: (def.args)(&Launch {
                port: cfg.port,
                data: &data,
            }),
            env: Vec::new(),
            cwd: Some(data),
            log_file: self.log_file(def),
            restart: RestartPolicy::OnFailure {
                max_restarts: 2,
                backoff: Duration::from_secs(1),
            },
        };
        self.inner.supervisor.start(spec).await?;
        if !health::wait_tcp(cfg.port, READY_TIMEOUT).await {
            let _ = self.inner.supervisor.stop(def.id, STOP_TIMEOUT).await;
            return Err(Error::Service(
                def.id.into(),
                format!(
                    "{} did not start listening on port {}.\n{}",
                    def.name,
                    cfg.port,
                    logs::tail(&self.log_file(def), 15)
                ),
            ));
        }
        tracing::info!("{} {} on 127.0.0.1:{}", def.name, install.version, cfg.port);
        Ok(ExtraRuntime {
            version: install.version,
            port: cfg.port,
        })
    }

    pub async fn stop(&self, def: &Definition) -> Result<()> {
        self.inner.supervisor.stop(def.id, STOP_TIMEOUT).await?;
        if let Some(state) = self.inner.state.lock().await.get_mut(def.id) {
            state.runtime = None;
            state.last_error = None;
        }
        Ok(())
    }

    pub async fn is_running(&self, def: &Definition) -> bool {
        self.inner.supervisor.info(def.id).await.is_some()
    }

    /// The running service's PID and port, for the wake check.
    pub async fn probe_target(&self, def: &Definition) -> Option<(u32, u16)> {
        let info = self.inner.supervisor.info(def.id).await?;
        if info.state != ProcessState::Running {
            return None;
        }
        let state = self.inner.state.lock().await.get(def.id).cloned()?;
        Some((info.pid, state.runtime?.port))
    }

    pub async fn status(&self, services: &BTreeMap<String, ExtraConfig>) -> Vec<ExtraStatus> {
        let states = self.inner.state.lock().await.clone();
        let mut out = Vec::new();
        for def in DEFINITIONS {
            let cfg = settings(services, def);
            let state = states.get(def.id).cloned().unwrap_or_default();
            let info = self.inner.supervisor.info(def.id).await;
            out.push(ExtraStatus {
                id: def.id.into(),
                name: def.name.into(),
                status: crate::service_status(info.as_ref(), state.last_error.is_some()),
                runtime: info.as_ref().and(state.runtime),
                pid: info.as_ref().map(|i| i.pid),
                error: state.last_error,
                log_file: self.log_file(def),
                versions: self
                    .versions(def)
                    .await
                    .into_iter()
                    .map(|i| i.version)
                    .collect(),
                connect: def.connect.replace("{port}", &cfg.port.to_string()),
                web_ui: def
                    .web_ui
                    .map(|u| u.replace("{port}", &cfg.port.to_string())),
                enabled: cfg.enabled,
                version: cfg.version,
                port: cfg.port,
            });
        }
        out
    }
}

/// The first word that looks like a version: "db version v9.0.4" → "9.0.4",
/// "Valkey server v=9.0.6 sha=..." → "9.0.6", "version 30GB 4.48 ..." → "4.48".
fn parse_version(text: &str) -> Option<String> {
    text.split(|c: char| c.is_whitespace() || c == '=' || c == ',')
        .map(|w| w.trim_start_matches('v'))
        .find(|w| {
            w.contains('.')
                && w.starts_with(|c: char| c.is_ascii_digit())
                && w.chars().all(|c| c.is_ascii_digit() || c == '.')
        })
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_each_services_version_banner() {
        for (text, version) in [
            ("db version v9.0.4\nBuild Info: {", "9.0.4"),
            (
                "Valkey server v=9.0.6 sha=00000000:0 malloc=jemalloc",
                "9.0.6",
            ),
            ("meilisearch 1.55.0", "1.55.0"),
            ("Typesense 30.2", "30.2"),
            ("version 30GB 4.48 530be3e windows amd64", "4.48"),
        ] {
            assert_eq!(parse_version(text).as_deref(), Some(version), "{text}");
        }
    }

    #[test]
    fn defaults_fill_missing_settings_and_ports_stay_local() {
        let services = BTreeMap::new();
        for def in DEFINITIONS {
            let cfg = settings(&services, def);
            assert_eq!(cfg.port, def.default_port);
            assert!(!cfg.enabled);
            let args: Vec<String> = (def.args)(&Launch {
                port: cfg.port,
                data: Path::new("data"),
            })
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
            assert!(
                args.iter().any(|a| a.contains("127.0.0.1")),
                "{} listens on loopback only: {args:?}",
                def.id
            );
        }
    }
}
