//! Recent requests per site, from the web server's JSON access log
//! (`logs/requests.log`, one line per request; see nginx.conf.tera and
//! httpd.conf.tera). Read from the end, so a large log costs nothing.

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Read at most this much of the log's end.
const TAIL_BYTES: u64 = 1024 * 1024;
/// The log is set aside (to `.1`) when the web server starts past this size.
const ROTATE_BYTES: u64 = 20 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    /// ISO 8601, as the web server wrote it.
    pub time: String,
    pub host: String,
    pub method: String,
    pub uri: String,
    pub status: u16,
    /// Time to answer, in milliseconds.
    pub ms: f64,
    pub bytes: u64,
}

/// One log line: nginx writes `method`, `uri` and `seconds`; Apache the
/// request line (`GET /cart?x=1 HTTP/1.1`) and `micros`.
#[derive(Deserialize)]
struct Line {
    time: String,
    host: String,
    #[serde(default)]
    method: String,
    #[serde(default)]
    uri: String,
    #[serde(default)]
    request: Option<String>,
    status: u16,
    #[serde(default)]
    seconds: Option<f64>,
    #[serde(default)]
    micros: Option<f64>,
    #[serde(default)]
    bytes: u64,
}

pub fn log_file(logs: &Path) -> PathBuf {
    logs.join("requests.log")
}

/// Sets a large log aside before the web server starts (it holds the file
/// open while it runs, so it cannot be moved then on Windows).
pub fn rotate(path: &Path) {
    if std::fs::metadata(path).is_ok_and(|m| m.len() > ROTATE_BYTES) {
        let old = path.with_extension("log.1");
        let _ = std::fs::remove_file(&old);
        if let Err(err) = std::fs::rename(path, &old) {
            tracing::warn!("could not set {} aside: {err}", path.display());
        }
    }
}

/// The newest `limit` requests for `domain` and its subdomains, newest first.
pub fn recent(path: &Path, domain: &str, limit: usize) -> Vec<Request> {
    let Ok(mut file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    let start = len.saturating_sub(TAIL_BYTES);
    if file.seek(SeekFrom::Start(start)).is_err() {
        return Vec::new();
    }
    let mut bytes = Vec::new();
    if file.read_to_end(&mut bytes).is_err() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&bytes);
    let mut lines = text.lines();
    if start > 0 {
        lines.next(); // probably cut in the middle
    }
    let lines: Vec<&str> = lines.collect();
    lines
        .iter()
        .rev()
        .filter_map(|l| serde_json::from_str::<Line>(l).ok())
        .map(|mut l| {
            if let Some(request) = l.request.take() {
                let mut parts = request.split_whitespace();
                l.method = parts.next().unwrap_or_default().to_owned();
                l.uri = parts.next().unwrap_or_default().to_owned();
            }
            l
        })
        .filter_map(|l| {
            let host = l
                .host
                .split(':')
                .next()
                .unwrap_or_default()
                .to_ascii_lowercase();
            let matches = host == domain || host.ends_with(&format!(".{domain}"));
            matches.then(|| Request {
                ms: l
                    .seconds
                    .map(|s| s * 1000.0)
                    .or(l.micros.map(|m| m / 1000.0))
                    .unwrap_or(0.0),
                time: l.time,
                host,
                method: l.method,
                uri: l.uri,
                status: l.status,
                bytes: l.bytes,
            })
        })
        .take(limit)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_both_servers_lines_newest_first_for_one_site() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let log = tmp.path().join("requests.log");
        std::fs::write(
            &log,
            concat!(
                r#"{"time":"2026-10-18T10:00:00+05:30","host":"blog.test","method":"GET","uri":"/","status":200,"seconds":0.012,"bytes":512}"#,
                "\n",
                "not json\n",
                r#"{"time":"2026-10-18T10:00:01+0530","host":"shop.test:8080","request":"POST /cart HTTP/1.1","status":302,"micros":4500,"bytes":0}"#,
                "\n",
                r#"{"time":"2026-10-18T10:00:02+05:30","host":"api.blog.test","method":"GET","uri":"/v1?x=1","status":500,"seconds":0.25,"bytes":10}"#,
                "\n",
            ),
        )
        .expect("write");

        let blog = recent(&log, "blog.test", 10);
        assert_eq!(blog.len(), 2);
        assert_eq!(blog[0].uri, "/v1?x=1", "newest first, subdomains included");
        assert!((blog[0].ms - 250.0).abs() < 1e-9);
        assert_eq!(blog[1].status, 200);

        let shop = recent(&log, "shop.test", 10);
        assert_eq!(shop.len(), 1, "the port is ignored");
        assert!((shop[0].ms - 4.5).abs() < 1e-9);
        assert_eq!(
            (shop[0].method.as_str(), shop[0].uri.as_str()),
            ("POST", "/cart")
        );
        assert!(recent(&tmp.path().join("missing.log"), "blog.test", 10).is_empty());
    }
}
