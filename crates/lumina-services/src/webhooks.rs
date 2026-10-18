//! The webhook catcher: a small HTTP endpoint on `127.0.0.1:<port>` that
//! records every request it gets (method, path, headers, body), so payment,
//! Git or chat webhooks can be looked at and sent to a site again later.
//!
//! With a site to forward to, each request also goes on to that site through
//! the web server and its answer goes back to the caller, so the catcher can
//! sit in front of a site and record what reaches it.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use lumina_core::{Error, Result};
use serde::Serialize;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

/// Requests kept, oldest dropped first.
const KEEP: usize = 200;
/// Larger bodies are cut (and marked as cut).
const MAX_BODY: usize = 5 * 1024 * 1024;
const MAX_HEAD: usize = 64 * 1024;
const FORWARD_TIMEOUT: Duration = Duration::from_secs(60);

/// Where requests are sent: a site, through the web server on `http_port`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub domain: String,
    pub http_port: u16,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Hook {
    pub id: u64,
    /// Unix milliseconds.
    pub received_at: u64,
    pub method: String,
    /// Path and query.
    pub path: String,
    pub headers: Vec<(String, String)>,
    /// As text (invalid UTF-8 shown with replacement characters).
    pub body: String,
    pub size: usize,
    pub truncated: bool,
    /// Status of the last forward or replay, or why it failed.
    pub forwarded: Option<String>,
    #[serde(skip)]
    raw: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WebhooksStatus {
    /// Set while the catcher listens.
    pub port: Option<u16>,
    /// The site requests are forwarded to as they arrive.
    pub forward_to: Option<String>,
    /// Newest first.
    pub hooks: Vec<Hook>,
}

#[derive(Clone, Default)]
pub struct Webhooks {
    inner: Arc<Inner>,
}

/// The listening port, the forward target and the accept loop.
type Server = (u16, Option<Target>, JoinHandle<()>);

#[derive(Default)]
struct Inner {
    hooks: Mutex<VecDeque<Hook>>,
    server: Mutex<Option<Server>>,
    next_id: AtomicU64,
    http: std::sync::OnceLock<reqwest::Client>,
}

impl Inner {
    fn http(&self) -> &reqwest::Client {
        self.http.get_or_init(|| {
            reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(FORWARD_TIMEOUT)
                .build()
                .unwrap_or_default()
        })
    }
}

impl Webhooks {
    /// Listens on `port` (restarting if it already listens), forwarding to
    /// `forward` if given.
    pub async fn start(&self, port: u16, forward: Option<Target>) -> Result<()> {
        self.stop().await;
        crate::web::check_port(port)?;
        let listener = TcpListener::bind(("127.0.0.1", port)).await.map_err(|e| {
            Error::Service(
                "webhooks".into(),
                format!("cannot listen on port {port}: {e}"),
            )
        })?;
        let inner = Arc::clone(&self.inner);
        let target = forward.clone();
        let task = tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    continue;
                };
                let inner = Arc::clone(&inner);
                let target = target.clone();
                tokio::spawn(async move {
                    if let Err(err) = serve(&inner, stream, target.as_ref()).await {
                        tracing::debug!("webhook connection: {err}");
                    }
                });
            }
        });
        tracing::info!("webhook catcher on 127.0.0.1:{port}");
        *self.inner.server.lock().await = Some((port, forward, task));
        Ok(())
    }

    pub async fn stop(&self) {
        if let Some((_, _, task)) = self.inner.server.lock().await.take() {
            task.abort();
            // Wait until the listener is dropped, so the port is free again.
            let _ = task.await;
        }
    }

    pub async fn status(&self) -> WebhooksStatus {
        let server = self.inner.server.lock().await;
        WebhooksStatus {
            port: server.as_ref().map(|(p, _, _)| *p),
            forward_to: server
                .as_ref()
                .and_then(|(_, t, _)| t.as_ref().map(|t| t.domain.clone())),
            hooks: self
                .inner
                .hooks
                .lock()
                .await
                .iter()
                .rev()
                .cloned()
                .collect(),
        }
    }

    pub async fn clear(&self) {
        self.inner.hooks.lock().await.clear();
    }

    /// Sends recorded request `id` to `target` again; returns its status.
    pub async fn replay(&self, id: u64, target: &Target) -> Result<u16> {
        let hook = self
            .inner
            .hooks
            .lock()
            .await
            .iter()
            .find(|h| h.id == id)
            .cloned()
            .ok_or_else(|| Error::Other(format!("no recorded webhook {id}")))?;
        let result = forward(&self.inner, &hook, target).await;
        let note = match &result {
            Ok((status, _)) => format!("{status} from {}", target.domain),
            Err(err) => err.to_string(),
        };
        if let Some(h) = self
            .inner
            .hooks
            .lock()
            .await
            .iter_mut()
            .find(|h| h.id == id)
        {
            h.forwarded = Some(note);
        }
        result.map(|(status, _)| status)
    }
}

async fn serve(
    inner: &Arc<Inner>,
    stream: TcpStream,
    target: Option<&Target>,
) -> std::io::Result<()> {
    let (read, mut write) = stream.into_split();
    let mut reader = BufReader::new(read);
    let mut head = String::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).await? == 0 {
            return Ok(());
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        head.push_str(&line);
        if head.len() > MAX_HEAD {
            return write
                .write_all(
                    b"HTTP/1.1 431 Request Header Fields Too Large\r\nConnection: close\r\n\r\n",
                )
                .await;
        }
    }
    let mut lines = head.lines();
    let mut first = lines.next().unwrap_or_default().split_whitespace();
    let method = first.next().unwrap_or("GET").to_owned();
    let path = first.next().unwrap_or("/").to_owned();
    let headers: Vec<(String, String)> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
        .collect();
    let header = |name: &str| {
        headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    };

    let mut body = Vec::new();
    let mut truncated = false;
    if header("transfer-encoding").is_some_and(|v| v.eq_ignore_ascii_case("chunked")) {
        loop {
            let mut size = String::new();
            reader.read_line(&mut size).await?;
            let size = usize::from_str_radix(size.trim().split(';').next().unwrap_or("0"), 16)
                .unwrap_or(0);
            if size == 0 {
                let mut end = String::new();
                reader.read_line(&mut end).await?;
                break;
            }
            let mut chunk = vec![0; size];
            reader.read_exact(&mut chunk).await?;
            let mut crlf = [0; 2];
            reader.read_exact(&mut crlf).await?;
            body.extend_from_slice(&chunk);
            if body.len() > MAX_BODY {
                truncated = true;
                body.truncate(MAX_BODY);
                break;
            }
        }
    } else if let Some(len) = header("content-length").and_then(|v| v.parse::<usize>().ok()) {
        let take = len.min(MAX_BODY);
        truncated = len > MAX_BODY;
        body = vec![0; take];
        reader.read_exact(&mut body).await?;
    }

    let id = inner.next_id.fetch_add(1, Ordering::SeqCst) + 1;
    let mut hook = Hook {
        id,
        received_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0),
        method,
        path,
        headers,
        body: String::from_utf8_lossy(&body).into_owned(),
        size: body.len(),
        truncated,
        forwarded: None,
        raw: body,
    };

    let reply = match target {
        Some(target) => match forward(inner, &hook, target).await {
            Ok((status, answer)) => {
                hook.forwarded = Some(format!("{status} from {}", target.domain));
                (status, answer)
            }
            Err(err) => {
                hook.forwarded = Some(err.to_string());
                (
                    502,
                    format!("Lumina could not forward this webhook: {err}\n").into_bytes(),
                )
            }
        },
        None => (200, b"{\"received\":true}\n".to_vec()),
    };
    {
        let mut hooks = inner.hooks.lock().await;
        hooks.push_back(hook);
        while hooks.len() > KEEP {
            hooks.pop_front();
        }
    }
    let (status, answer) = reply;
    let head = format!(
        "HTTP/1.1 {status} {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        reason(status),
        answer.len()
    );
    write.write_all(head.as_bytes()).await?;
    write.write_all(&answer).await?;
    write.shutdown().await
}

/// Sends `hook` to `target` through the web server; returns status and body.
async fn forward(inner: &Inner, hook: &Hook, target: &Target) -> Result<(u16, Vec<u8>)> {
    let url = format!("http://127.0.0.1:{}{}", target.http_port, hook.path);
    let method = reqwest::Method::from_bytes(hook.method.as_bytes())
        .map_err(|_| Error::Other(format!("bad method {}", hook.method)))?;
    let mut request = inner
        .http()
        .request(method, url)
        .header("Host", &target.domain);
    for (name, value) in &hook.headers {
        let skip = ["host", "content-length", "connection", "transfer-encoding"];
        if !skip.iter().any(|s| name.eq_ignore_ascii_case(s)) {
            request = request.header(name, value);
        }
    }
    let response = request
        .body(hook.raw.clone())
        .send()
        .await
        .map_err(|e| Error::Other(format!("forward to {}: {e}", target.domain)))?;
    let status = response.status().as_u16();
    let body = response
        .bytes()
        .await
        .map(|b| b.to_vec())
        .unwrap_or_default();
    Ok((status, body))
}

fn reason(status: u16) -> &'static str {
    reqwest::StatusCode::from_u16(status)
        .ok()
        .and_then(|s| s.canonical_reason())
        .unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;

    fn free_port() -> u16 {
        std::net::TcpListener::bind("127.0.0.1:0")
            .expect("bind")
            .local_addr()
            .expect("addr")
            .port()
    }

    async fn send(port: u16, raw: &[u8]) -> String {
        let mut stream = TcpStream::connect(("127.0.0.1", port))
            .await
            .expect("connect");
        stream.write_all(raw).await.expect("write");
        let mut answer = String::new();
        stream.read_to_string(&mut answer).await.expect("read");
        answer
    }

    #[tokio::test]
    async fn records_requests_and_forwards_them_to_a_site() {
        // A stand-in for the web server: answers with the path and host it got.
        let site = TcpListener::bind("127.0.0.1:0").await.expect("site");
        let site_port = site.local_addr().expect("addr").port();
        tokio::spawn(async move {
            loop {
                let (mut s, _) = site.accept().await.expect("accept");
                let mut buf = vec![0; 4096];
                let n = s.read(&mut buf).await.unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let host = req
                    .lines()
                    .find_map(|l| l.strip_prefix("host: ").or(l.strip_prefix("Host: ")))
                    .unwrap_or_default()
                    .to_owned();
                let body = format!(
                    "site saw {} on {host}",
                    req.lines().next().unwrap_or_default()
                );
                let reply = format!(
                    "HTTP/1.1 201 Created\r\nContent-Length: {}\r\n\r\n{body}",
                    body.len()
                );
                let _ = s.write_all(reply.as_bytes()).await;
            }
        });

        let hooks = Webhooks::default();
        let port = free_port();
        hooks.start(port, None).await.expect("start");
        let answer = send(
            port,
            b"POST /stripe?x=1 HTTP/1.1\r\nHost: x\r\nStripe-Signature: t=1\r\nContent-Length: 11\r\n\r\n{\"ok\":true}",
        )
        .await;
        assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");
        let chunked = send(
            port,
            b"POST /chunked HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n",
        )
        .await;
        assert!(chunked.starts_with("HTTP/1.1 200"), "{chunked}");

        let status = hooks.status().await;
        assert_eq!(status.port, Some(port));
        assert_eq!(status.hooks.len(), 2);
        assert_eq!(
            status.hooks[0].body, "hello world",
            "newest first, chunks joined"
        );
        let stripe = &status.hooks[1];
        assert_eq!(
            (stripe.method.as_str(), stripe.path.as_str()),
            ("POST", "/stripe?x=1")
        );
        assert_eq!(stripe.body, r#"{"ok":true}"#);
        assert!(
            stripe
                .headers
                .iter()
                .any(|(k, v)| k == "Stripe-Signature" && v == "t=1")
        );

        let target = Target {
            domain: "shop.test".into(),
            http_port: site_port,
        };
        assert_eq!(hooks.replay(stripe.id, &target).await.expect("replay"), 201);
        let replayed = hooks
            .status()
            .await
            .hooks
            .into_iter()
            .find(|h| h.id == stripe.id)
            .expect("hook");
        assert_eq!(replayed.forwarded.as_deref(), Some("201 from shop.test"));

        // Forwarding mode: the caller gets the site's answer.
        hooks.start(port, Some(target)).await.expect("restart");
        let answer = send(port, b"POST /hook HTTP/1.1\r\nContent-Length: 0\r\n\r\n").await;
        assert!(answer.starts_with("HTTP/1.1 201"), "{answer}");
        assert!(
            answer.contains("site saw POST /hook HTTP/1.1 on shop.test"),
            "{answer}"
        );

        hooks.stop().await;
        assert_eq!(hooks.status().await.port, None);
    }
}
