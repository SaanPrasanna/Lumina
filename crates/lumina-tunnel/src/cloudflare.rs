//! Cloudflare quick tunnels: `cloudflared tunnel --url ...` gives a random
//! `https://<words>.trycloudflare.com` address with no account and no token.
//! One `cloudflared` process per shared site (`cloudflared:<name>` under the
//! supervisor), logging to `logs/cloudflared-<name>.log`.
//!
//! cloudflared rewrites `Host` to the site's local domain, sends the public
//! address in `X-Forwarded-Host` and marks every request with
//! `Cf-Warp-Tag-Id`; the web server templates treat that header like
//! [`crate::ngrok::HEADER`], so PHP sees the public host and HTTPS.
//!
//! Quick tunnels use HTTP/2 to Cloudflare: the default (QUIC on UDP 7844) is
//! blocked by many routers and office networks.

use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Mutex as StdMutex, MutexGuard};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use lumina_process::{ProcessSpec, ProcessState, RestartPolicy, Supervisor, logs};
use tokio::sync::Mutex;
use tokio::time::{Instant, sleep};

use crate::provider::{Tunnel, TunnelProvider, TunnelSpec};
use crate::{Error, Result};

pub const PROCESS_PREFIX: &str = "cloudflared:";
/// The address appears, and the first connection registers, within seconds;
/// slow networks get some slack.
const READY_TIMEOUT: Duration = Duration::from_secs(60);
const STOP_TIMEOUT: Duration = Duration::from_secs(10);

pub fn program_name() -> String {
    format!("cloudflared{}", std::env::consts::EXE_SUFFIX)
}

/// What `open` needs: the installed cloudflared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub program: PathBuf,
}

pub struct Cloudflare {
    supervisor: Supervisor,
    logs_dir: PathBuf,
    ops: Mutex<()>,
    tunnels: StdMutex<BTreeMap<String, Tunnel>>,
}

impl Cloudflare {
    pub fn new(supervisor: Supervisor, logs_dir: &Path) -> Self {
        Self {
            supervisor,
            logs_dir: logs_dir.to_path_buf(),
            ops: Mutex::new(()),
            tunnels: StdMutex::new(BTreeMap::new()),
        }
    }

    pub fn log_file(&self, name: &str) -> PathBuf {
        self.logs_dir.join(format!("cloudflared-{name}.log"))
    }

    fn state(&self) -> MutexGuard<'_, BTreeMap<String, Tunnel>> {
        self.tunnels.lock().unwrap_or_else(|e| e.into_inner())
    }

    async fn alive(&self, name: &str) -> bool {
        self.supervisor
            .info(&format!("{PROCESS_PREFIX}{name}"))
            .await
            .is_some_and(|i| i.state == ProcessState::Running)
    }
}

/// cloudflared's command line for `spec`.
pub fn args(spec: &TunnelSpec) -> Vec<std::ffi::OsString> {
    [
        "tunnel".to_owned(),
        "--no-autoupdate".to_owned(),
        "--protocol".to_owned(),
        "http2".to_owned(),
        "--url".to_owned(),
        format!("http://127.0.0.1:{}", spec.port),
        "--http-host-header".to_owned(),
        spec.host.clone(),
    ]
    .into_iter()
    .map(Into::into)
    .collect()
}

/// The quick-tunnel address in cloudflared's log output, once the first
/// connection to Cloudflare is registered (before that the address answers
/// with an error page).
pub fn ready_url(log: &str) -> Option<String> {
    if !log.contains("Registered tunnel connection") {
        return None;
    }
    let start = log.find("https://")?;
    log[start..]
        .split(|c: char| c.is_whitespace() || c == '|')
        .next()
        .filter(|u| u.ends_with(".trycloudflare.com"))
        .map(str::to_owned)
}

pub fn blocked(log: &str) -> bool {
    log.contains("precheck complete hard_fail=true")
}

const BLOCKED: &str = "this network blocks Cloudflare Tunnel (outbound port 7844); share with ngrok instead, or try another network";

/// The log written since byte `from` (this run only).
fn log_since(path: &Path, from: u64) -> String {
    let Ok(mut file) = std::fs::File::open(path) else {
        return String::new();
    };
    if file.seek(SeekFrom::Start(from)).is_err() {
        return String::new();
    }
    let mut text = String::new();
    let _ = file.read_to_string(&mut text);
    text
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl TunnelProvider for Cloudflare {
    type Settings = Settings;

    async fn open(&self, settings: &Settings, spec: &TunnelSpec) -> Result<Tunnel> {
        let _ops = self.ops.lock().await;
        if spec.domain.is_some() {
            return Err(Error::Invalid(
                "Cloudflare quick tunnels get a random address; a reserved domain needs ngrok"
                    .into(),
            ));
        }
        let existing = self.state().get(&spec.name).cloned();
        if let Some(tunnel) = existing {
            if self.alive(&spec.name).await {
                return Ok(tunnel);
            }
            self.state().remove(&spec.name);
        }

        let id = format!("{PROCESS_PREFIX}{}", spec.name);
        let log = self.log_file(&spec.name);
        let from = std::fs::metadata(&log).map(|m| m.len()).unwrap_or(0);
        self.supervisor
            .start(ProcessSpec {
                id: id.clone(),
                program: settings.program.clone(),
                args: args(spec),
                env: Vec::new(),
                cwd: None,
                log_file: log.clone(),
                restart: RestartPolicy::Never,
            })
            .await?;

        let deadline = Instant::now() + READY_TIMEOUT;
        let url = loop {
            if let Some(url) = ready_url(&log_since(&log, from)) {
                break url;
            }
            let running = self
                .supervisor
                .info(&id)
                .await
                .is_some_and(|i| i.state == ProcessState::Running);
            let text = log_since(&log, from);
            if blocked(&text) {
                let _ = self.supervisor.stop(&id, STOP_TIMEOUT).await;
                return Err(Error::Invalid(BLOCKED.into()));
            }
            if !running || Instant::now() >= deadline {
                let _ = self.supervisor.stop(&id, STOP_TIMEOUT).await;
                if text.contains("port 7844") {
                    return Err(Error::Invalid(BLOCKED.into()));
                }
                return Err(Error::Invalid(format!(
                    "cloudflared did not open a tunnel for {}.\n{}",
                    spec.host,
                    logs::tail(&log, 12)
                )));
            }
            sleep(Duration::from_millis(250)).await;
        };
        tracing::info!("{} shared at {url} (Cloudflare)", spec.host);
        let tunnel = Tunnel {
            name: spec.name.clone(),
            public_url: url,
            host: spec.host.clone(),
            port: spec.port,
            started_at: unix_now(),
        };
        self.state().insert(spec.name.clone(), tunnel.clone());
        Ok(tunnel)
    }

    async fn close(&self, name: &str) -> Result<()> {
        let _ops = self.ops.lock().await;
        self.supervisor
            .stop(&format!("{PROCESS_PREFIX}{name}"), STOP_TIMEOUT)
            .await?;
        self.state().remove(name);
        Ok(())
    }

    async fn tunnels(&self) -> Vec<Tunnel> {
        let all: Vec<Tunnel> = self.state().values().cloned().collect();
        let mut out = Vec::new();
        for tunnel in all {
            if self.alive(&tunnel.name).await {
                out.push(tunnel);
            } else {
                self.state().remove(&tunnel.name);
            }
        }
        out
    }

    async fn shutdown(&self) -> Result<()> {
        let names: Vec<String> = self.state().keys().cloned().collect();
        let mut first = None;
        for name in names {
            if let Err(err) = self.close(&name).await {
                first.get_or_insert(err);
            }
        }
        first.map_or(Ok(()), Err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_address_once_a_connection_is_registered() {
        let start = "2026-10-17T21:20:01Z INF |  https://straight-accurate-ultra-easy.trycloudflare.com                                     |\n";
        assert_eq!(ready_url(start), None, "not registered yet");
        let ready = format!(
            "{start}2026-10-17T21:20:03Z INF Registered tunnel connection connIndex=0 protocol=http2\n"
        );
        assert_eq!(
            ready_url(&ready).as_deref(),
            Some("https://straight-accurate-ultra-easy.trycloudflare.com")
        );
        let other = "INF Registered tunnel connection\nhttps://developers.cloudflare.com/x\n";
        assert_eq!(ready_url(other), None);
    }

    #[test]
    fn notices_a_network_that_blocks_cloudflare() {
        let log = "2026-10-18T05:40:20Z INF |  ERROR: Allow outbound TCP on port 7844.  |
                   2026-10-18T05:40:20Z INF precheck complete hard_fail=true run_id=90a5c756
";
        assert!(blocked(log));
        assert!(!blocked(
            "2026-10-18T05:40:20Z INF precheck complete hard_fail=false run_id=1
"
        ));
    }

    #[test]
    fn forwards_to_the_site_over_http2() {
        let spec = TunnelSpec {
            name: "blog".into(),
            host: "blog.test".into(),
            port: 8080,
            domain: None,
        };
        let args: Vec<String> = args(&spec)
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(
            args.windows(2)
                .any(|w| w == ["--url", "http://127.0.0.1:8080"])
        );
        assert!(
            args.windows(2)
                .any(|w| w == ["--http-host-header", "blog.test"])
        );
        assert!(args.windows(2).any(|w| w == ["--protocol", "http2"]));
    }
}
