//! Ngrok v3 agent: authtoken, start tunnel with host-header rewrite, read public URL from its local API.
//!
//! One agent runs while at least one site is shared (free ngrok accounts
//! allow a single agent session, which then carries several tunnels). It is
//! started with a generated config file that holds no secrets — the
//! authtoken comes from the system keychain through `NGROK_AUTHTOKEN` — and
//! tunnels are added and removed through its local API, so sharing another
//! site never restarts the ones already open.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Mutex as StdMutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use lumina_process::{ProcessSpec, ProcessState, RestartPolicy, Supervisor, logs, ports};
use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::provider::{Tunnel, TunnelProvider, TunnelSpec};
use crate::{Error, Result};

/// Supervisor ID of the agent.
pub const PROCESS_ID: &str = "ngrok";
/// Added to every request that comes through a tunnel; the web server
/// templates then hand PHP the public host (`X-Forwarded-Host`) and HTTPS.
pub const HEADER: &str = "X-Lumina-Tunnel";
/// The agent normally answers within a second; connecting to ngrok's
/// servers can take a few more on a slow network.
const START_LIMIT: Duration = Duration::from_secs(20);
const POLL: Duration = Duration::from_millis(200);
const STOP_LIMIT: Duration = Duration::from_secs(5);
/// Marks the start of a run in the log (see `lumina_process::logs`).
const RUN_MARKER: &str = "===== Lumina: ";

/// `ngrok` / `ngrok.exe`, at the top of an installed version's folder.
pub fn program_name() -> String {
    format!("ngrok{}", std::env::consts::EXE_SUFFIX)
}

/// Read fresh for every `open`: a new token or ngrok version restarts the agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub program: PathBuf,
    pub authtoken: String,
}

pub struct Ngrok {
    supervisor: Supervisor,
    config_file: PathBuf,
    log_file: PathBuf,
    http: reqwest::Client,
    /// Serializes open / close / shutdown.
    ops: Mutex<()>,
    state: StdMutex<State>,
}

#[derive(Default)]
struct State {
    agent: Option<Agent>,
    tunnels: BTreeMap<String, Tunnel>,
    /// Why the agent stopped on its own, until the next `open`.
    stopped: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Agent {
    api_port: u16,
    settings: Settings,
}

/// The answer to "open this tunnel".
enum Post {
    Opened(String),
    /// The agent has not connected to ngrok yet (or cannot be reached).
    NotReady,
    /// The agent already has a tunnel by this name (e.g. left behind by a
    /// "not ready" answer).
    Exists,
    Refused(Error),
}

impl Ngrok {
    /// `config_dir` is `etc/ngrok`; the agent logs to `<logs_dir>/ngrok.log`.
    pub fn new(supervisor: Supervisor, config_dir: &Path, logs_dir: &Path) -> Self {
        let http = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default();
        Self {
            supervisor,
            config_file: config_dir.join("ngrok.yml"),
            log_file: logs_dir.join("ngrok.log"),
            http,
            ops: Mutex::new(()),
            state: StdMutex::new(State::default()),
        }
    }

    pub fn log_file(&self) -> &Path {
        &self.log_file
    }

    /// Why the agent stopped by itself (e.g. the token was revoked), if it did.
    pub fn stopped_reason(&self) -> Option<String> {
        self.state().stopped.clone()
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    async fn alive(&self) -> bool {
        self.supervisor
            .info(PROCESS_ID)
            .await
            .is_some_and(|i| i.state == ProcessState::Running)
    }

    /// The agent's API port, starting the agent if needed.
    async fn agent(&self, settings: &Settings) -> Result<u16> {
        let current = self.state().agent.clone();
        if let Some(agent) = current {
            if self.alive().await && agent.settings == *settings {
                return Ok(agent.api_port);
            }
            self.stop_agent().await?;
        }

        let api_port = api_port();
        if let Some(dir) = self.config_file.parent() {
            std::fs::create_dir_all(dir).map_err(|source| Error::Io {
                path: dir.to_path_buf(),
                source,
            })?;
        }
        std::fs::write(&self.config_file, render_config(api_port)).map_err(|source| Error::Io {
            path: self.config_file.clone(),
            source,
        })?;
        let args = [
            "start".into(),
            "--none".into(),
            "--config".into(),
            self.config_file.clone().into_os_string(),
            "--log".into(),
            "stdout".into(),
            "--log-format".into(),
            "json".into(),
        ];
        self.supervisor
            .start(ProcessSpec {
                id: PROCESS_ID.to_owned(),
                program: settings.program.clone(),
                args: args.to_vec(),
                env: vec![(
                    OsString::from("NGROK_AUTHTOKEN"),
                    OsString::from(&settings.authtoken),
                )],
                cwd: self.config_file.parent().map(Path::to_path_buf),
                log_file: self.log_file.clone(),
                restart: RestartPolicy::Never,
            })
            .await?;
        {
            let mut state = self.state();
            state.agent = Some(Agent {
                api_port,
                settings: settings.clone(),
            });
            state.stopped = None;
        }

        let deadline = Instant::now() + START_LIMIT;
        loop {
            if !self.alive().await {
                let err = self.failure();
                let _ = self.stop_agent().await;
                return Err(err);
            }
            // Tunnels are asked for only once the agent is connected: asked
            // earlier, it answers "not ready" but keeps the name, and the
            // tunnel never starts.
            let api_up = self.list(api_port).await.is_ok();
            if api_up && session_established(&logs::tail(&self.log_file, 400)) {
                return Ok(api_port);
            }
            if Instant::now() >= deadline {
                let _ = self.stop_agent().await;
                if !api_up {
                    return Err(Error::Agent(format!(
                        "its local API did not answer on port {api_port}"
                    )));
                }
                let detail = match failure_from_log(&logs::tail(&self.log_file, 400)) {
                    Some(Error::Agent(m) | Error::Auth(m)) => m,
                    _ => "check the internet connection".into(),
                };
                return Err(Error::Connect(format!(
                    "no answer within {} seconds ({detail})",
                    START_LIMIT.as_secs()
                )));
            }
            tokio::time::sleep(POLL).await;
        }
    }

    async fn stop_agent(&self) -> Result<()> {
        {
            let mut state = self.state();
            state.agent = None;
            state.tunnels.clear();
        }
        self.supervisor.stop(PROCESS_ID, STOP_LIMIT).await?;
        Ok(())
    }

    /// Why the agent exited, from this run's log.
    fn failure(&self) -> Error {
        failure_from_log(&logs::tail(&self.log_file, 400)).unwrap_or_else(|| {
            Error::Agent("it exited without saying why; see the ngrok log".into())
        })
    }

    fn api(port: u16, path: &str) -> String {
        format!("http://127.0.0.1:{port}{path}")
    }

    async fn list(&self, port: u16) -> Result<Value> {
        let resp = self
            .http
            .get(Self::api(port, "/api/tunnels"))
            .send()
            .await
            .map_err(|e| Error::Agent(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(Error::Agent(format!("API answered {}", resp.status())));
        }
        resp.json().await.map_err(|e| Error::Agent(e.to_string()))
    }

    async fn post(&self, port: u16, body: &Value) -> Post {
        let Ok(resp) = self
            .http
            .post(Self::api(port, "/api/tunnels"))
            .json(body)
            .send()
            .await
        else {
            return Post::NotReady;
        };
        let status = resp.status();
        let value: Value = resp.json().await.unwrap_or(Value::Null);
        if status.is_success() {
            return match value["public_url"].as_str() {
                Some(url) if !url.is_empty() => Post::Opened(url.to_owned()),
                _ => Post::Refused(Error::Agent(
                    "ngrok opened the tunnel but sent no public address".into(),
                )),
            };
        }
        let err = value["details"]["err"]
            .as_str()
            .or_else(|| value["msg"].as_str())
            .unwrap_or_default();
        // No session yet: 503 with code 104 ("not yet ready to start
        // tunnels") right after start, 502 "session has not yet been
        // established" while the agent cannot reach ngrok.
        if value["error_code"].as_u64() == Some(104)
            || status.as_u16() == 503
            || err.contains("has not yet been established")
        {
            return Post::NotReady;
        }
        if err.contains("already exists") {
            return Post::Exists;
        }
        Post::Refused(api_error(&value, status.as_u16()))
    }

    async fn delete(&self, port: u16, name: &str) {
        let _ = self
            .http
            .delete(Self::api(port, &format!("/api/tunnels/{name}")))
            .send()
            .await;
    }

    async fn open_locked(&self, settings: &Settings, spec: &TunnelSpec) -> Result<Tunnel> {
        let existing = self.state().tunnels.get(&spec.name).cloned();
        if let Some(t) = &existing
            && self.alive().await
            && t.host == spec.host
            && t.port == spec.port
        {
            return Ok(t.clone());
        }
        let api = self.agent(settings).await?;
        if existing.is_some() {
            // Same site, new port or domain: replace the old tunnel.
            self.delete(api, &spec.name).await;
            self.state().tunnels.remove(&spec.name);
        }

        let body = tunnel_body(spec);
        let deadline = Instant::now() + START_LIMIT;
        // A "not ready" answer leaves the name behind without starting the
        // tunnel; a later attempt then hears "already exists". Remove the
        // leftover and ask again (a few times at most).
        let mut leftovers = 0;
        loop {
            match self.post(api, &body).await {
                Post::Exists if leftovers < 3 => {
                    leftovers += 1;
                    self.delete(api, &spec.name).await;
                }
                Post::Exists => {
                    return Err(Error::Api(format!(
                        "a tunnel named `{}` already exists in the agent",
                        spec.name
                    )));
                }
                Post::Opened(public_url) => {
                    let tunnel = Tunnel {
                        name: spec.name.clone(),
                        public_url,
                        host: spec.host.clone(),
                        port: spec.port,
                        started_at: unix_now(),
                    };
                    tracing::info!("shared {} at {}", spec.host, tunnel.public_url);
                    self.state()
                        .tunnels
                        .insert(spec.name.clone(), tunnel.clone());
                    return Ok(tunnel);
                }
                Post::Refused(err) => return Err(err),
                Post::NotReady if !self.alive().await => return Err(self.failure()),
                Post::NotReady if Instant::now() >= deadline => {
                    // Still retrying in the background: say what it is stuck on.
                    let detail = match failure_from_log(&logs::tail(&self.log_file, 400)) {
                        Some(Error::Agent(m) | Error::Auth(m)) => m,
                        _ => "check the internet connection".into(),
                    };
                    return Err(Error::Connect(format!(
                        "no answer within {} seconds ({detail})",
                        START_LIMIT.as_secs()
                    )));
                }
                Post::NotReady => tokio::time::sleep(POLL).await,
            }
        }
    }
}

impl TunnelProvider for Ngrok {
    type Settings = Settings;

    async fn open(&self, settings: &Settings, spec: &TunnelSpec) -> Result<Tunnel> {
        validate_spec(spec)?;
        let _op = self.ops.lock().await;
        let result = self.open_locked(settings, spec).await;
        if result.is_err() && self.state().tunnels.is_empty() {
            // Nothing else is shared: do not keep an idle session open.
            let _ = self.stop_agent().await;
        }
        result
    }

    async fn close(&self, name: &str) -> Result<()> {
        let _op = self.ops.lock().await;
        let agent = {
            let mut state = self.state();
            if state.tunnels.remove(name).is_none() {
                return Ok(());
            }
            if !state.tunnels.is_empty() {
                state.agent.clone()
            } else {
                None
            }
        };
        match agent {
            Some(agent) => {
                self.delete(agent.api_port, name).await;
                tracing::info!("stopped sharing {name}");
                Ok(())
            }
            // That was the last one: the agent goes with it.
            None => {
                tracing::info!("stopped sharing {name}; stopping the ngrok agent");
                self.stop_agent().await
            }
        }
    }

    async fn tunnels(&self) -> Vec<Tunnel> {
        if self.state().agent.is_none() {
            return Vec::new();
        }
        // An `open` or `close` in progress handles (and reports) the agent's
        // exit itself: only a stop nobody asked for is noted here.
        let Ok(_op) = self.ops.try_lock() else {
            return self.state().tunnels.values().cloned().collect();
        };
        if !self.alive().await {
            // Exited by itself (a revoked token, a session limit...).
            let reason = self.failure().to_string();
            tracing::warn!("ngrok agent stopped: {reason}");
            {
                let mut state = self.state();
                state.agent = None;
                state.tunnels.clear();
                state.stopped = Some(reason);
            }
            let _ = self.supervisor.stop(PROCESS_ID, STOP_LIMIT).await;
            return Vec::new();
        }
        self.state().tunnels.values().cloned().collect()
    }

    async fn shutdown(&self) -> Result<()> {
        let _op = self.ops.lock().await;
        if self.state().agent.is_none() {
            return Ok(());
        }
        self.stop_agent().await
    }
}

/// 4040 (ngrok's usual port), or the next free one.
fn api_port() -> u16 {
    (4040..4050)
        .find(|p| ports::is_free(*p))
        .or_else(|| {
            std::net::TcpListener::bind("127.0.0.1:0")
                .and_then(|l| l.local_addr())
                .map(|a| a.port())
                .ok()
        })
        .unwrap_or(4040)
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The agent's config file: its API address, no prompts, no secrets.
pub fn render_config(api_port: u16) -> String {
    format!(
        "# Generated by Lumina each time the ngrok agent starts; do not edit.\n\
         # The authtoken is not stored here: Lumina passes it from the system\n\
         # keychain. Tunnels are opened through the agent API below.\n\
         version: 3\n\
         agent:\n\
         \x20 web_addr: 127.0.0.1:{api_port}\n\
         \x20 console_ui: false\n\
         \x20 update_check: false\n"
    )
}

/// The tunnel definition sent to `POST /api/tunnels`.
pub fn tunnel_body(spec: &TunnelSpec) -> Value {
    let mut body = json!({
        "name": spec.name,
        "proto": "http",
        "addr": format!("127.0.0.1:{}", spec.port),
        "host_header": spec.host,
        "inspect": false,
        "request_header": { "add": [format!("{HEADER}: 1")] },
    });
    if let Some(domain) = &spec.domain {
        body["domain"] = Value::String(domain.clone());
    }
    body
}

fn validate_spec(spec: &TunnelSpec) -> Result<()> {
    let name_ok = !spec.name.is_empty()
        && spec
            .name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !name_ok {
        return Err(Error::Invalid(format!(
            "`{}` cannot name a tunnel",
            spec.name
        )));
    }
    if !is_hostname(&spec.host) {
        return Err(Error::Invalid(format!(
            "`{}` is not a host name",
            spec.host
        )));
    }
    if spec.port == 0 {
        return Err(Error::Invalid("the web server has no HTTP port".into()));
    }
    if let Some(domain) = &spec.domain
        && !is_hostname(domain)
    {
        return Err(Error::Invalid(format!("`{domain}` is not a domain")));
    }
    Ok(())
}

fn is_hostname(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 253
        && s.contains('.')
        && s.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
}

/// A reserved domain as typed (`https://My-App.ngrok.app/`) → `my-app.ngrok.app`;
/// empty → `None`.
pub fn normalize_domain(input: &str) -> Result<Option<String>> {
    let s = input.trim().to_ascii_lowercase();
    let s = s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
        .unwrap_or(&s)
        .trim_end_matches('/');
    if s.is_empty() {
        return Ok(None);
    }
    if !is_hostname(s) {
        return Err(Error::Invalid(format!(
            "`{}` is not a domain",
            input.trim()
        )));
    }
    Ok(Some(s.to_owned()))
}

/// Whether this run of the agent has connected to ngrok.
pub fn session_established(log: &str) -> bool {
    let run = log.rsplit_once(RUN_MARKER).map_or(log, |(_, run)| run);
    run.lines().any(|line| {
        serde_json::from_str::<Value>(line.trim())
            .is_ok_and(|r| r["msg"] == "client session established")
    })
}

/// Why this run of the agent failed: its last critical record, else its last
/// error. `None` if the log does not say.
pub fn failure_from_log(log: &str) -> Option<Error> {
    let run = log.rsplit_once(RUN_MARKER).map_or(log, |(_, run)| run);
    let mut crit = None;
    let mut error = None;
    for line in run.lines() {
        let Ok(record) = serde_json::from_str::<Value>(line.trim()) else {
            continue;
        };
        let Some(err) = record["err"].as_str().filter(|e| !e.is_empty()) else {
            continue;
        };
        match record["lvl"].as_str() {
            Some("crit") => crit = Some(err.to_owned()),
            Some("eror") => error = Some(err.to_owned()),
            _ => {}
        }
    }
    crit.or(error).map(|raw| classify(&raw))
}

/// An API error answer (`{ error_code, msg, details: { err } }`).
fn api_error(value: &Value, status: u16) -> Error {
    let msg = value["msg"].as_str().unwrap_or_default();
    match value["details"]["err"].as_str().filter(|e| !e.is_empty()) {
        Some(err) => match classify(err) {
            Error::Agent(m) => Error::Api(m),
            other => other,
        },
        None if !msg.is_empty() => Error::Api(msg.to_owned()),
        None => Error::Api(format!("the agent answered {status}")),
    }
}

/// ngrok's error codes for an unusable authtoken.
const AUTH_CODES: [&str; 4] = ["105", "106", "107", "4018"];

/// Auth problems apart from the rest, with a one-line message.
fn classify(raw: &str) -> Error {
    let code = error_code(raw);
    let auth = raw.contains("authentication failed")
        || code.as_deref().is_some_and(|c| AUTH_CODES.contains(&c));
    let mut message = summary(raw);
    if auth {
        message = message
            .strip_prefix("authentication failed: ")
            .map(str::to_owned)
            .unwrap_or(message);
    }
    if let Some(code) = code {
        message = format!("{message} (ERR_NGROK_{code})");
    }
    if auth {
        Error::Auth(message)
    } else {
        Error::Agent(message)
    }
}

/// The first meaningful line. ngrok echoes a rejected token back
/// ("Your authtoken: ..."); that line is never repeated.
fn summary(raw: &str) -> String {
    raw.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .filter(|l| !l.starts_with("Your authtoken"))
        .find(|l| !l.starts_with("ERR_NGROK_"))
        .unwrap_or("unknown error")
        .to_owned()
}

/// `4018` from "... ERR_NGROK_4018 ...".
fn error_code(raw: &str) -> Option<String> {
    let rest = &raw[raw.find("ERR_NGROK_")? + "ERR_NGROK_".len()..];
    let code: String = rest.chars().take_while(char::is_ascii_digit).collect();
    (!code.is_empty()).then_some(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> TunnelSpec {
        TunnelSpec {
            name: "blog".into(),
            host: "blog.test".into(),
            port: 8080,
            domain: None,
        }
    }

    #[test]
    fn the_tunnel_rewrites_the_host_and_marks_requests() {
        let body = tunnel_body(&spec());
        assert_eq!(body["addr"], "127.0.0.1:8080");
        assert_eq!(body["host_header"], "blog.test");
        assert_eq!(body["proto"], "http");
        assert_eq!(body["request_header"]["add"][0], "X-Lumina-Tunnel: 1");
        assert!(body.get("domain").is_none());

        let reserved = TunnelSpec {
            domain: Some("my-app.ngrok.app".into()),
            ..spec()
        };
        assert_eq!(tunnel_body(&reserved)["domain"], "my-app.ngrok.app");
    }

    #[test]
    fn the_config_has_no_secrets() {
        let config = render_config(4041);
        assert!(config.contains("version: 3\n"));
        assert!(config.contains("  web_addr: 127.0.0.1:4041\n"));
        assert!(!config.contains("authtoken:"));
    }

    #[test]
    fn specs_are_checked() {
        assert!(validate_spec(&spec()).is_ok());
        for bad in [
            TunnelSpec {
                name: "../x".into(),
                ..spec()
            },
            TunnelSpec {
                host: "blog test".into(),
                ..spec()
            },
            TunnelSpec { port: 0, ..spec() },
            TunnelSpec {
                domain: Some("not a domain".into()),
                ..spec()
            },
        ] {
            assert!(validate_spec(&bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn domains_are_normalized() {
        assert_eq!(
            normalize_domain(" https://My-App.ngrok.app/ ").expect("ok"),
            Some("my-app.ngrok.app".into())
        );
        assert_eq!(normalize_domain("").expect("ok"), None);
        assert!(normalize_domain("my app").is_err());
        assert!(normalize_domain("localhost").is_err());
    }

    /// Real records from ngrok 3.39 (no token, then a malformed one).
    #[test]
    fn failures_are_read_from_this_run_only() {
        let log = concat!(
            "\n===== Lumina: ngrok start =====\n",
            r#"{"lvl":"crit","msg":"command failed","err":"an older run\n"}"#,
            "\n\n===== Lumina: ngrok start =====\n",
            r#"{"t":"2026-10-15T06:36:54+0530","lvl":"info","msg":"open config file","path":"x","err":null}"#,
            "\n",
            r#"{"t":"2026-10-15T06:36:56+0530","lvl":"eror","msg":"failed to reconnect session","obj":"tunnels.session","err":"authentication failed: This ngrok session is not authenticated. ngrok requires an account and a valid credential to start a session.\n\nSign up for an account: https://dashboard.ngrok.com/signup\nGet your credential: https://dashboard.ngrok.com/get-started/your-authtoken\r\n\r\nERR_NGROK_4018\r\n"}"#,
            "\nERROR:  authentication failed: plain-text copy\n",
        );
        match failure_from_log(log) {
            Some(Error::Auth(m)) => assert_eq!(
                m,
                "This ngrok session is not authenticated. ngrok requires an account and a valid credential to start a session. (ERR_NGROK_4018)"
            ),
            other => panic!("{other:?}"),
        }

        let bad_token = concat!(
            "===== Lumina: ngrok start =====\n",
            r#"{"lvl":"crit","msg":"command failed","err":"authentication failed: The authtoken you specified does not look like a proper ngrok authtoken.\nYour authtoken: 2abcdefBOGUS\nInstructions to install your authtoken are on your ngrok dashboard:\nhttps://dashboard.ngrok.com/get-started/your-authtoken\r\n\r\nERR_NGROK_105\r\n"}"#,
        );
        let err = failure_from_log(bad_token).expect("failure");
        assert!(matches!(err, Error::Auth(_)));
        assert!(!err.to_string().contains("BOGUS"), "{err}");
        assert!(err.to_string().ends_with("(ERR_NGROK_105)"), "{err}");

        assert!(failure_from_log("===== Lumina: x =====\nnot json\n").is_none());
    }

    /// Real records from ngrok 3.39: an earlier run connected, this one not yet.
    #[test]
    fn the_session_is_read_from_this_run_only() {
        let connected = r#"{"t":"2026-10-15T07:51:40+0530","lvl":"info","msg":"client session established","obj":"tunnels.session"}"#;
        let starting = r#"{"t":"2026-10-15T07:51:38+0530","lvl":"info","msg":"starting web service","obj":"web","addr":"127.0.0.1:4040","allow_hosts":null}"#;
        let log = format!(
            "{RUN_MARKER}ngrok start =====
{connected}

{RUN_MARKER}ngrok start =====
{starting}
"
        );
        assert!(!session_established(&log));
        assert!(session_established(&format!(
            "{log}{connected}
"
        )));
    }

    #[test]
    fn api_errors_keep_the_reason() {
        let value = json!({
            "error_code": 102,
            "status_code": 400,
            "msg": "invalid tunnel configuration",
            "details": { "err": "failed to start tunnel: The endpoint 'https://x.ngrok.app' is not reserved.\r\n\r\nERR_NGROK_320\r\n" }
        });
        match api_error(&value, 400) {
            Error::Api(m) => assert_eq!(
                m,
                "failed to start tunnel: The endpoint 'https://x.ngrok.app' is not reserved. (ERR_NGROK_320)"
            ),
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            api_error(&json!({ "msg": "nope" }), 500),
            Error::Api(m) if m == "nope"
        ));
    }
}
