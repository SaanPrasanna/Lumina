//! Mailpit: catches every mail sent to `127.0.0.1:<smtp_port>` and shows it in
//! an inbox at `http://127.0.0.1:<ui_port>`. Messages are kept in
//! `data/mailpit/mailpit.db`. Any SMTP username and password is accepted, so
//! app settings copied from production work unchanged.
//!
//! PHP's `mail()` reaches it too (see [`php_ini`]): on Windows through PHP's
//! built-in SMTP client, elsewhere through `mailpit sendmail`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use lumina_config::schema::Mailpit as MailpitConfig;
use lumina_core::paths::Layout;
use lumina_core::types::ServiceStatus;
use lumina_core::{Error, Result};
use lumina_process::{ProcessSpec, ProcessState, RestartPolicy, Supervisor, health, logs};
use serde::Serialize;
use tokio::sync::Mutex;

use crate::install::{self, Install};
use crate::web;

/// Supervisor ID.
pub const PROCESS_ID: &str = "mailpit";
const READY_TIMEOUT: Duration = Duration::from_secs(20);
const STOP_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MailpitRuntime {
    pub version: String,
    pub smtp_port: u16,
    pub ui_port: u16,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MailpitStatus {
    pub status: ServiceStatus,
    pub runtime: Option<MailpitRuntime>,
    pub pid: Option<u32>,
    pub error: Option<String>,
    pub log_file: PathBuf,
}

#[derive(Clone)]
pub struct Mailpit {
    inner: Arc<Inner>,
}

struct Inner {
    supervisor: Supervisor,
    layout: Layout,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    runtime: Option<MailpitRuntime>,
    last_error: Option<String>,
}

impl Mailpit {
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
        self.inner.layout.bin().join("mailpit")
    }

    pub fn log_file(&self) -> PathBuf {
        self.inner.layout.logs().join("mailpit.log")
    }

    /// Installed versions, newest first.
    pub async fn versions(&self) -> Vec<Install> {
        install::discover(
            &self.root(),
            "mailpit",
            &["mailpit", "bin/mailpit"],
            &["version"],
            parse_version,
        )
        .await
    }

    /// The install `cfg` selects, if there is one.
    pub async fn selected(&self, cfg: &MailpitConfig) -> Option<Install> {
        install::select(&self.versions().await, &cfg.version).cloned()
    }

    pub async fn start(&self, cfg: &MailpitConfig) -> Result<MailpitRuntime> {
        let mut state = self.inner.state.lock().await;
        let result = self.start_locked(&mut state, cfg).await;
        state.last_error = result.as_ref().err().map(ToString::to_string);
        result
    }

    async fn start_locked(&self, state: &mut State, cfg: &MailpitConfig) -> Result<MailpitRuntime> {
        if let Some(info) = self.inner.supervisor.info(PROCESS_ID).await
            && info.state != ProcessState::Failed
        {
            return Err(Error::Service(PROCESS_ID.into(), "already running".into()));
        }
        let install = self.selected(cfg).await.ok_or_else(|| {
            install::not_installed(PROCESS_ID, "Mailpit", &cfg.version, &self.root())
        })?;

        web::check_port(cfg.smtp_port)?;
        web::check_port(cfg.ui_port)?;
        let data = self.inner.layout.data().join("mailpit");
        std::fs::create_dir_all(&data).map_err(|source| Error::Io {
            path: data.clone(),
            source,
        })?;
        let spec = ProcessSpec {
            id: PROCESS_ID.into(),
            program: install.exe.clone(),
            args: args(cfg, &data.join("mailpit.db")),
            env: Vec::new(),
            cwd: Some(data),
            log_file: self.log_file(),
            restart: RestartPolicy::OnFailure {
                max_restarts: 2,
                backoff: Duration::from_secs(1),
            },
        };
        self.inner.supervisor.start(spec).await?;

        let ready = health::wait_tcp(cfg.smtp_port, READY_TIMEOUT).await
            && health::wait_tcp(cfg.ui_port, READY_TIMEOUT).await;
        if !ready {
            let _ = self.inner.supervisor.stop(PROCESS_ID, STOP_TIMEOUT).await;
            return Err(Error::Service(
                PROCESS_ID.into(),
                format!(
                    "Mailpit did not start listening on ports {} and {}.\n{}",
                    cfg.smtp_port,
                    cfg.ui_port,
                    logs::tail(&self.log_file(), 10)
                ),
            ));
        }

        let runtime = MailpitRuntime {
            version: install.version.clone(),
            smtp_port: cfg.smtp_port,
            ui_port: cfg.ui_port,
        };
        tracing::info!(
            "Mailpit {} catching mail on 127.0.0.1:{}, inbox on port {}",
            install.version,
            cfg.smtp_port,
            cfg.ui_port
        );
        state.runtime = Some(runtime.clone());
        Ok(runtime)
    }

    /// Mailpit saves each message as it arrives, so a plain stop loses nothing.
    pub async fn stop(&self) -> Result<()> {
        let mut state = self.inner.state.lock().await;
        self.inner.supervisor.stop(PROCESS_ID, STOP_TIMEOUT).await?;
        state.runtime = None;
        Ok(())
    }

    pub async fn status(&self) -> MailpitStatus {
        let state = self.inner.state.lock().await;
        let info = self.inner.supervisor.info(PROCESS_ID).await;
        MailpitStatus {
            status: crate::service_status(info.as_ref(), state.last_error.is_some()),
            runtime: info.as_ref().and(state.runtime.clone()),
            pid: info.as_ref().map(|i| i.pid),
            error: state.last_error.clone(),
            log_file: self.log_file(),
        }
    }
}

/// Command line for the server: local only, any SMTP login accepted, no
/// update check (Lumina's catalog handles updates).
fn args(cfg: &MailpitConfig, database: &Path) -> Vec<std::ffi::OsString> {
    vec![
        "--smtp".into(),
        format!("127.0.0.1:{}", cfg.smtp_port).into(),
        "--listen".into(),
        format!("127.0.0.1:{}", cfg.ui_port).into(),
        "--database".into(),
        database.as_os_str().to_owned(),
        "--smtp-auth-accept-any".into(),
        "--smtp-auth-allow-insecure".into(),
        "--disable-version-check".into(),
    ]
}

/// php.ini settings (`-d name="value"`) that send PHP's `mail()` to Mailpit.
/// On Windows PHP talks SMTP itself; elsewhere `mail()` runs a sendmail
/// program, which `mailpit sendmail` stands in for. `exe` is the installed
/// Mailpit; without one, nothing changes outside Windows.
pub fn php_ini(cfg: &MailpitConfig, exe: Option<&Path>) -> Vec<(String, String)> {
    if !cfg.php_mail {
        return Vec::new();
    }
    if cfg!(windows) {
        return vec![
            ("SMTP".into(), "127.0.0.1".into()),
            ("smtp_port".into(), cfg.smtp_port.to_string()),
        ];
    }
    // PHP runs sendmail_path through the shell: the path is single-quoted
    // there, and the whole value double-quoted for php.ini.
    let Some(exe) = exe.map(|e| e.to_string_lossy().into_owned()) else {
        return Vec::new();
    };
    if exe.contains(['\'', '"']) {
        tracing::warn!("PHP mail is not sent to Mailpit: its path contains a quote ({exe})");
        return Vec::new();
    }
    vec![(
        "sendmail_path".into(),
        format!("'{exe}' sendmail -S 127.0.0.1:{}", cfg.smtp_port),
    )]
}

/// "mailpit v1.31.6 compiled with go1.27.2 on windows/amd64" → "1.31.6".
fn parse_version(text: &str) -> Option<String> {
    let rest = &text[text.find(" v")? + 2..];
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
            parse_version(r"C:\Lumina\bin\mailpit\1.31.6\mailpit.exe v1.31.6 compiled with go1.27.2 on windows/amd64")
                .as_deref(),
            Some("1.31.6")
        );
        assert_eq!(parse_version("nothing here"), None);
    }

    #[test]
    fn listens_on_loopback_only() {
        let cfg = MailpitConfig::default();
        let args: Vec<String> = args(&cfg, Path::new("m.db"))
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(args.windows(2).any(|w| w == ["--smtp", "127.0.0.1:1025"]));
        assert!(args.windows(2).any(|w| w == ["--listen", "127.0.0.1:8025"]));
    }

    #[test]
    fn php_mail_goes_to_mailpit_unless_turned_off() {
        let mut cfg = MailpitConfig {
            smtp_port: 2525,
            ..MailpitConfig::default()
        };
        let exe = Path::new("/opt/lumina/bin/mailpit/1.31.6/mailpit");
        let ini = php_ini(&cfg, Some(exe));
        if cfg!(windows) {
            assert_eq!(
                ini,
                [
                    ("SMTP".to_owned(), "127.0.0.1".to_owned()),
                    ("smtp_port".to_owned(), "2525".to_owned())
                ]
            );
        } else {
            assert_eq!(ini.len(), 1);
            assert_eq!(ini[0].0, "sendmail_path");
            assert_eq!(
                ini[0].1,
                "'/opt/lumina/bin/mailpit/1.31.6/mailpit' sendmail -S 127.0.0.1:2525"
            );
            assert!(php_ini(&cfg, None).is_empty(), "no Mailpit, no change");
        }
        cfg.php_mail = false;
        assert!(php_ini(&cfg, Some(exe)).is_empty());
    }
}
