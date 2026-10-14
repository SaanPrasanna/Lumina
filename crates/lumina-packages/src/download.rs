//! Resumable downloads with progress events.
//!
//! Data goes to `<file>.part`. If a `.part` file is already there (an earlier
//! attempt was interrupted or cancelled), the download continues from its end
//! with an HTTP `Range` request; a server that ignores the range sends the
//! whole file again and the partial data is discarded. Only a complete file is
//! renamed to its final name.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use reqwest::StatusCode;
use reqwest::header::{CONTENT_RANGE, RANGE};
use tokio::io::AsyncWriteExt;

use crate::{Error, Result};

/// Set to stop a download; the `.part` file is kept for resuming.
pub type Cancel = Arc<AtomicBool>;

pub struct Downloader {
    client: reqwest::Client,
}

impl Downloader {
    /// `proxy`: `http://host:port` to use instead of the system's settings.
    pub fn new(proxy: Option<&str>) -> Result<Self> {
        let mut builder = reqwest::Client::builder()
            .user_agent(concat!("Lumina/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(20))
            .read_timeout(Duration::from_secs(60));
        if let Some(proxy) = proxy.map(str::trim).filter(|p| !p.is_empty()) {
            builder = builder.proxy(
                reqwest::Proxy::all(proxy)
                    .map_err(|e| Error::Download(format!("proxy {proxy}: {e}")))?,
            );
        }
        let client = builder
            .build()
            .map_err(|e| Error::Download(e.to_string()))?;
        Ok(Self { client })
    }

    /// Downloads the first URL that works into `dest` and returns it.
    /// `progress(received, total)` is called as data arrives.
    pub async fn fetch(
        &self,
        urls: &[&str],
        dest: &Path,
        cancel: &Cancel,
        progress: &(dyn Fn(u64, Option<u64>) + Send + Sync),
    ) -> Result<PathBuf> {
        let mut last = None;
        for url in urls {
            match self.fetch_one(url, dest, cancel, progress).await {
                Ok(path) => return Ok(path),
                Err(Error::Cancelled) => return Err(Error::Cancelled),
                Err(err) => {
                    tracing::warn!("download from {url} failed: {err}");
                    last = Some(err);
                }
            }
        }
        Err(last.unwrap_or_else(|| Error::Download("no download URL".into())))
    }

    async fn fetch_one(
        &self,
        url: &str,
        dest: &Path,
        cancel: &Cancel,
        progress: &(dyn Fn(u64, Option<u64>) + Send + Sync),
    ) -> Result<PathBuf> {
        let part = part_path(dest);
        if let Some(dir) = part.parent() {
            tokio::fs::create_dir_all(dir)
                .await
                .map_err(|source| Error::Io {
                    path: dir.to_path_buf(),
                    source,
                })?;
        }
        let have = tokio::fs::metadata(&part)
            .await
            .map(|m| m.len())
            .unwrap_or(0);

        let mut request = self.client.get(url);
        if have > 0 {
            request = request.header(RANGE, format!("bytes={have}-"));
        }
        let mut resp = request
            .send()
            .await
            .map_err(|e| Error::Download(format!("{url}: {e}")))?;

        let status = resp.status();
        let (mut received, append) = match status {
            StatusCode::PARTIAL_CONTENT if have > 0 && range_starts_at(&resp, have) => (have, true),
            // The .part file already holds everything.
            StatusCode::RANGE_NOT_SATISFIABLE if have > 0 => {
                return finish(&part, dest).await;
            }
            s if s.is_success() => (0, false),
            s => return Err(Error::Download(format!("{url}: HTTP {s}"))),
        };
        let total = resp.content_length().map(|len| len + received);
        if append {
            tracing::info!("resuming {url} at {received} bytes");
        }

        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(append)
            .truncate(!append)
            .open(&part)
            .await
            .map_err(|source| Error::Io {
                path: part.clone(),
                source,
            })?;
        progress(received, total);

        loop {
            if cancel.load(Ordering::Relaxed) {
                file.flush().await.ok();
                return Err(Error::Cancelled);
            }
            let chunk = resp.chunk().await.map_err(|e| {
                Error::Download(format!(
                    "{url}: connection lost after {received} bytes ({e})"
                ))
            })?;
            let Some(chunk) = chunk else { break };
            file.write_all(&chunk).await.map_err(|source| Error::Io {
                path: part.clone(),
                source,
            })?;
            received += chunk.len() as u64;
            progress(received, total);
        }
        file.flush().await.map_err(|source| Error::Io {
            path: part.clone(),
            source,
        })?;
        drop(file);

        if let Some(total) = total
            && received < total
        {
            return Err(Error::Download(format!(
                "{url}: connection closed after {received} of {total} bytes"
            )));
        }
        finish(&part, dest).await
    }
}

/// `<dest>.part`.
pub fn part_path(dest: &Path) -> PathBuf {
    let mut name = dest.as_os_str().to_owned();
    name.push(".part");
    PathBuf::from(name)
}

fn range_starts_at(resp: &reqwest::Response, offset: u64) -> bool {
    // "bytes 1000-1999/2000"
    resp.headers()
        .get(CONTENT_RANGE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("bytes "))
        .and_then(|v| v.split('-').next())
        .and_then(|start| start.parse::<u64>().ok())
        == Some(offset)
}

async fn finish(part: &Path, dest: &Path) -> Result<PathBuf> {
    tokio::fs::rename(part, dest)
        .await
        .map_err(|source| Error::Io {
            path: dest.to_path_buf(),
            source,
        })?;
    Ok(dest.to_path_buf())
}

#[cfg(test)]
pub(crate) mod test_server {
    //! A tiny HTTP/1.1 file server for tests: supports `Range`, can cut the
    //! first response short, and can ignore ranges.

    use std::net::SocketAddr;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    pub struct Server {
        pub addr: SocketAddr,
        pub ranges: Arc<std::sync::Mutex<Vec<String>>>,
    }

    /// Serves `body` at any path. The first response stops after `cut_at`
    /// bytes if set. `honour_range = false` always sends the whole file.
    pub async fn serve(body: Vec<u8>, cut_at: Option<usize>, honour_range: bool) -> Server {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let count = Arc::new(AtomicUsize::new(0));
        let ranges = Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen = ranges.clone();
        let body = Arc::new(body);
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else {
                    break;
                };
                let n = count.fetch_add(1, Ordering::SeqCst);
                let body = body.clone();
                let seen = seen.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 4096];
                    let len = sock.read(&mut buf).await.unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..len]).to_string();
                    let range = req
                        .lines()
                        .find_map(|l| {
                            l.strip_prefix("range: ")
                                .or_else(|| l.strip_prefix("Range: "))
                        })
                        .map(str::to_owned);
                    let start = range
                        .as_deref()
                        .filter(|_| honour_range)
                        .and_then(|r| r.strip_prefix("bytes="))
                        .and_then(|r| r.trim_end_matches('-').parse::<usize>().ok());
                    seen.lock().expect("lock").push(range.unwrap_or_default());
                    let head = match start {
                        Some(s) if s >= body.len() => {
                            let _ = sock.write_all(b"HTTP/1.1 416 Range Not Satisfiable\r\ncontent-length: 0\r\nconnection: close\r\n\r\n").await;
                            return;
                        }
                        Some(s) => format!(
                            "HTTP/1.1 206 Partial Content\r\ncontent-length: {}\r\ncontent-range: bytes {s}-{}/{}\r\nconnection: close\r\n\r\n",
                            body.len() - s,
                            body.len() - 1,
                            body.len()
                        ),
                        None => format!(
                            "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                            body.len()
                        ),
                    };
                    let from = start.unwrap_or(0);
                    let mut data = &body[from..];
                    if n == 0
                        && let Some(cut) = cut_at
                    {
                        data = &data[..cut.min(data.len())];
                    }
                    let _ = sock.write_all(head.as_bytes()).await;
                    let _ = sock.write_all(data).await;
                    let _ = sock.shutdown().await;
                });
            }
        });
        Server { addr, ranges }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body() -> Vec<u8> {
        (0..300_000u32).map(|i| (i % 251) as u8).collect()
    }

    #[tokio::test]
    async fn interrupted_download_resumes_with_a_range_request() {
        let server = test_server::serve(body(), Some(100_000), true).await;
        let tmp = tempfile::tempdir().expect("tempdir");
        let dest = tmp.path().join("a.zip");
        let url = format!("http://{}/a.zip", server.addr);
        let d = Downloader::new(None).expect("client");
        let cancel = Cancel::default();

        let first = d.fetch(&[&url], &dest, &cancel, &|_, _| {}).await;
        assert!(first.is_err(), "the cut connection must be reported");
        assert_eq!(
            std::fs::metadata(part_path(&dest)).expect("part").len(),
            100_000
        );

        let last = std::sync::Mutex::new((0, None));
        d.fetch(&[&url], &dest, &cancel, &|r, t| {
            *last.lock().expect("lock") = (r, t)
        })
        .await
        .expect("resume");
        assert_eq!(std::fs::read(&dest).expect("read"), body());
        assert!(!part_path(&dest).exists());
        assert_eq!(*last.lock().expect("lock"), (300_000, Some(300_000)));
        assert_eq!(server.ranges.lock().expect("lock")[1], "bytes=100000-");
    }

    #[tokio::test]
    async fn a_server_without_range_support_restarts_cleanly() {
        let server = test_server::serve(body(), Some(50_000), false).await;
        let tmp = tempfile::tempdir().expect("tempdir");
        let dest = tmp.path().join("a.zip");
        let url = format!("http://{}/a.zip", server.addr);
        let d = Downloader::new(None).expect("client");
        let cancel = Cancel::default();
        let _ = d.fetch(&[&url], &dest, &cancel, &|_, _| {}).await;
        d.fetch(&[&url], &dest, &cancel, &|_, _| {})
            .await
            .expect("restart");
        assert_eq!(std::fs::read(&dest).expect("read"), body());
    }

    #[tokio::test]
    async fn mirrors_are_tried_and_cancel_keeps_the_part_file() {
        let server = test_server::serve(body(), None, true).await;
        let tmp = tempfile::tempdir().expect("tempdir");
        let dest = tmp.path().join("a.zip");
        let good = format!("http://{}/a.zip", server.addr);
        let d = Downloader::new(None).expect("client");

        let cancel = Cancel::new(AtomicBool::new(true));
        let err = d
            .fetch(&[&good], &dest, &cancel, &|_, _| {})
            .await
            .expect_err("cancelled");
        assert!(matches!(err, Error::Cancelled));

        let cancel = Cancel::default();
        d.fetch(
            &["http://127.0.0.1:1/unreachable", &good],
            &dest,
            &cancel,
            &|_, _| {},
        )
        .await
        .expect("mirror");
        assert_eq!(std::fs::read(&dest).expect("read").len(), 300_000);
    }
}
