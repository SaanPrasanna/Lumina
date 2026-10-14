//! The helper's logic, kept in a library so it can be tested without elevation.
//!
//! One sub-command per run. Input is validated before anything is touched:
//! hosts entries must be lowercase DNS names pointing at `127.0.0.1` or `::1`,
//! and the request file must be a small regular JSON file with no unknown
//! fields. The helper only ever writes the hosts file's Lumina block (plus the
//! `hosts.lumina.bak` copy next to it).

use std::collections::HashSet;
use std::fmt;
use std::net::IpAddr;
use std::path::Path;

use lumina_platform::hosts::{self, Entry, HostsError};
use serde::Deserialize;

/// Upper bound for a request file; a few thousand entries fit easily.
pub const MAX_REQUEST_BYTES: u64 = 1024 * 1024;
pub const MAX_ENTRIES: usize = 5000;

/// Exit codes, shared with the app (`lumina_vhost::hosts_sync`). All below 64
/// so they never collide with pkexec's 126/127.
pub mod exit {
    pub const OK: i32 = 0;
    /// Unknown sub-command or wrong number of arguments.
    pub const USAGE: i32 = 2;
    /// The request failed validation; nothing was changed.
    pub const INVALID: i32 = 3;
    /// The hosts file has a broken Lumina block or an unreadable encoding.
    pub const HOSTS_FILE: i32 = 4;
    /// Reading or writing failed.
    pub const IO: i32 = 5;
}

#[derive(Debug)]
pub enum HelperError {
    Usage(String),
    Invalid(String),
    Hosts(HostsError),
    Io(String),
}

impl HelperError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Usage(_) => exit::USAGE,
            Self::Invalid(_) => exit::INVALID,
            Self::Hosts(_) => exit::HOSTS_FILE,
            Self::Io(_) => exit::IO,
        }
    }
}

impl fmt::Display for HelperError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(m) | Self::Invalid(m) | Self::Io(m) => f.write_str(m),
            Self::Hosts(e) => e.fmt(f),
        }
    }
}

/// `hosts-apply` request: the complete desired content of the Lumina block.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HostsRequest {
    entries: Vec<RawEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEntry {
    ip: String,
    host: String,
}

/// The validated entries of a `hosts-apply` request (JSON text).
pub fn parse_hosts_request(json: &str) -> Result<Vec<Entry>, HelperError> {
    let request: HostsRequest = serde_json::from_str(json)
        .map_err(|e| HelperError::Invalid(format!("malformed request: {e}")))?;
    if request.entries.len() > MAX_ENTRIES {
        return Err(HelperError::Invalid(format!(
            "too many entries ({} > {MAX_ENTRIES})",
            request.entries.len()
        )));
    }
    let mut seen = HashSet::new();
    let mut entries = Vec::with_capacity(request.entries.len());
    for raw in request.entries {
        // `IpAddr::from_str` accepts only canonical dotted/colon forms.
        let ip: IpAddr = raw
            .ip
            .parse()
            .map_err(|_| HelperError::Invalid(format!("not an IP address: {:?}", raw.ip)))?;
        if !hosts::is_allowed_ip(&ip) {
            return Err(HelperError::Invalid(format!(
                "only 127.0.0.1 and ::1 are allowed, got {ip}"
            )));
        }
        if !hosts::is_valid_host(&raw.host) {
            return Err(HelperError::Invalid(format!(
                "not a valid host name: {:?}",
                raw.host
            )));
        }
        let entry = Entry::new(ip, raw.host);
        if !seen.insert(entry.clone()) {
            return Err(HelperError::Invalid(format!(
                "duplicate entry: {} {}",
                entry.ip, entry.host
            )));
        }
        entries.push(entry);
    }
    Ok(entries)
}

/// Reads a request file: a regular file (not a link), at most
/// [`MAX_REQUEST_BYTES`], valid UTF-8.
pub fn read_request(path: &Path) -> Result<String, HelperError> {
    if !path.is_absolute() {
        return Err(HelperError::Usage(
            "the request path must be absolute".into(),
        ));
    }
    let meta = std::fs::symlink_metadata(path)
        .map_err(|e| HelperError::Io(format!("{}: {e}", path.display())))?;
    if !meta.file_type().is_file() {
        return Err(HelperError::Invalid(format!(
            "{} is not a regular file",
            path.display()
        )));
    }
    if meta.len() > MAX_REQUEST_BYTES {
        return Err(HelperError::Invalid("the request file is too large".into()));
    }
    let bytes =
        std::fs::read(path).map_err(|e| HelperError::Io(format!("{}: {e}", path.display())))?;
    String::from_utf8(bytes).map_err(|_| HelperError::Invalid("the request is not UTF-8".into()))
}

/// What `hosts-apply` did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applied {
    /// The block already had exactly these entries; the file was not touched.
    Unchanged,
    Written,
}

/// Replaces the Lumina block of `hosts_file` with `entries`.
pub fn apply_hosts(hosts_file: &Path, entries: &[Entry]) -> Result<Applied, HelperError> {
    let current = std::fs::read(hosts_file)
        .map_err(|e| HelperError::Io(format!("{}: {e}", hosts_file.display())))?;
    let updated = hosts::apply_block(&current, entries).map_err(HelperError::Hosts)?;
    if updated == current {
        return Ok(Applied::Unchanged);
    }
    hosts::write_hosts(hosts_file, &updated)
        .map_err(|e| HelperError::Io(format!("{}: {e}", hosts_file.display())))?;
    Ok(Applied::Written)
}

/// Runs one command line (without the program name) against `hosts_file`.
pub fn run(args: &[String], hosts_file: &Path) -> Result<(), HelperError> {
    match args {
        [cmd, request] if cmd == "hosts-apply" => {
            let entries = parse_hosts_request(&read_request(Path::new(request))?)?;
            if apply_hosts(hosts_file, &entries)? == Applied::Written {
                hosts::flush_dns();
            }
            Ok(())
        }
        // Later phases: ca-install, ca-remove, path-system-add, bind-cap.
        _ => Err(HelperError::Usage(
            "usage: lumina-helper hosts-apply <request.json>".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(json: &str) -> Vec<Entry> {
        parse_hosts_request(json).unwrap_or_else(|e| panic!("{json}: {e}"))
    }

    fn invalid(json: &str) {
        match parse_hosts_request(json) {
            Err(HelperError::Invalid(_)) => {}
            other => panic!("{json}: expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn accepts_loopback_entries() {
        let entries = ok(
            r#"{"entries":[{"ip":"127.0.0.1","host":"blog.test"},{"ip":"::1","host":"blog.test"}]}"#,
        );
        assert_eq!(entries.len(), 2);
        assert!(ok(r#"{"entries":[]}"#).is_empty());
    }

    #[test]
    fn rejects_everything_else() {
        for json in [
            "",
            "null",
            "[]",
            "{}",
            r#"{"entries":{}}"#,
            r#"{"entries":[],"extra":1}"#,
            r#"{"entries":[{"ip":"127.0.0.1"}]}"#,
            r#"{"entries":[{"ip":"127.0.0.1","host":"a.test","x":1}]}"#,
            r#"{"entries":[{"ip":"10.0.0.1","host":"a.test"}]}"#,
            r#"{"entries":[{"ip":"0.0.0.0","host":"a.test"}]}"#,
            r#"{"entries":[{"ip":"127.0.0.2","host":"a.test"}]}"#,
            r#"{"entries":[{"ip":"127.1","host":"a.test"}]}"#,
            r#"{"entries":[{"ip":" 127.0.0.1","host":"a.test"}]}"#,
            r#"{"entries":[{"ip":"127.0.0.1","host":"A.test"}]}"#,
            r#"{"entries":[{"ip":"127.0.0.1","host":"a.test\n10.0.0.1 bank.example"}]}"#,
            r#"{"entries":[{"ip":"127.0.0.1","host":"a.test # x"}]}"#,
            r#"{"entries":[{"ip":"127.0.0.1","host":""}]}"#,
            r#"{"entries":[{"ip":"127.0.0.1","host":"a.test"},{"ip":"127.0.0.1","host":"a.test"}]}"#,
            r#"{"entries":[{"ip":127,"host":"a.test"}]}"#,
        ] {
            invalid(json);
        }
        let many = format!(
            r#"{{"entries":[{}]}}"#,
            (0..=MAX_ENTRIES)
                .map(|i| format!(r#"{{"ip":"127.0.0.1","host":"s{i}.test"}}"#))
                .collect::<Vec<_>>()
                .join(",")
        );
        invalid(&many);
    }

    /// xorshift64*: deterministic, so a failure reproduces.
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }
        fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
            items[(self.next() % items.len() as u64) as usize]
        }
    }

    /// Fuzz: random hosts and IPs (mostly near-valid, to reach deep checks) and
    /// random mutations of valid JSON. Nothing may panic, and every accepted
    /// entry must satisfy the rules independently checked here.
    #[test]
    fn fuzz_never_accepts_bad_input() {
        let mut rng = Rng(0x5EED_1234_ABCD_0001);
        let pieces = [
            "a",
            "z",
            "0",
            "9",
            "-",
            ".",
            "test",
            "local",
            "A",
            "_",
            " ",
            "\t",
            "\n",
            "\r",
            "#",
            "\"",
            "\\",
            "/",
            ":",
            "é",
            "\u{0}",
            "..",
            "--",
            "xn--",
            "localhost",
        ];
        let ips = [
            "127.0.0.1",
            "::1",
            "0:0:0:0:0:0:0:1",
            "127.0.0.2",
            "0.0.0.0",
            "::",
            "10.0.0.1",
            "127.0.0.1 ",
            "127.0.0.01",
            "::ffff:127.0.0.1",
            "1.1.1.1",
            "",
            "x",
            "127.0.0.1\n",
        ];
        let mut accepted = 0;
        for _ in 0..20_000 {
            let host: String = (0..1 + rng.next() % 8).map(|_| rng.pick(&pieces)).collect();
            let ip = rng.pick(&ips);
            let mut json =
                serde_json::json!({ "entries": [{ "ip": ip, "host": host }] }).to_string();
            if rng.next().is_multiple_of(4) && !json.is_empty() {
                // Byte-level mutation: drop, duplicate or flip a byte.
                let mut bytes = json.into_bytes();
                let i = (rng.next() % bytes.len() as u64) as usize;
                match rng.next() % 3 {
                    0 => {
                        bytes.remove(i);
                    }
                    1 => bytes.insert(i, bytes[i]),
                    _ => bytes[i] ^= 1 << (rng.next() % 7),
                }
                json = String::from_utf8_lossy(&bytes).into_owned();
            }
            if let Ok(entries) = parse_hosts_request(&json) {
                for e in entries {
                    accepted += 1;
                    assert!(
                        e.ip.is_loopback() && e.ip.to_string() != "127.0.0.2",
                        "{json}"
                    );
                    assert!(
                        e.host.split('.').all(|l| !l.is_empty()
                            && l.bytes().all(|b| b.is_ascii_lowercase()
                                || b.is_ascii_digit()
                                || b == b'-')
                            && !l.starts_with('-')
                            && !l.ends_with('-')),
                        "{json}"
                    );
                }
            }
        }
        assert!(
            accepted > 100,
            "the fuzzer should also produce valid input ({accepted})"
        );
    }

    #[test]
    fn request_file_rules() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let file = tmp.path().join("req.json");
        std::fs::write(&file, r#"{"entries":[]}"#).expect("write");
        assert!(read_request(&file).is_ok());
        assert!(matches!(
            read_request(Path::new("req.json")),
            Err(HelperError::Usage(_))
        ));
        assert!(matches!(
            read_request(tmp.path()),
            Err(HelperError::Invalid(_))
        ));
        std::fs::write(&file, vec![b' '; (MAX_REQUEST_BYTES + 1) as usize]).expect("big");
        assert!(matches!(read_request(&file), Err(HelperError::Invalid(_))));
        std::fs::write(&file, [0xFF, 0xFE]).expect("bad utf8");
        assert!(matches!(read_request(&file), Err(HelperError::Invalid(_))));
    }

    #[test]
    fn run_applies_and_is_idempotent() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let hosts_file = tmp.path().join("hosts");
        std::fs::write(&hosts_file, "127.0.0.1 localhost\n").expect("hosts");
        let request = tmp.path().join("req.json");
        std::fs::write(
            &request,
            r#"{"entries":[{"ip":"127.0.0.1","host":"blog.test"}]}"#,
        )
        .expect("req");
        let args = vec![
            "hosts-apply".to_owned(),
            request.to_string_lossy().into_owned(),
        ];

        run(&args, &hosts_file).expect("run");
        let text = std::fs::read_to_string(&hosts_file).expect("read");
        assert!(
            text.starts_with("127.0.0.1 localhost\n# BEGIN LUMINA"),
            "{text}"
        );
        assert!(text.contains("blog.test"));
        assert!(tmp.path().join("hosts.lumina.bak").exists());

        let entries = parse_hosts_request(&read_request(&request).expect("read")).expect("parse");
        assert_eq!(
            apply_hosts(&hosts_file, &entries).expect("again"),
            Applied::Unchanged
        );

        assert!(matches!(
            run(&["nope".into()], &hosts_file),
            Err(HelperError::Usage(_))
        ));
        assert!(matches!(run(&[], &hosts_file), Err(HelperError::Usage(_))));
    }

    #[test]
    fn broken_block_is_reported_and_file_untouched() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let hosts_file = tmp.path().join("hosts");
        std::fs::write(&hosts_file, "# BEGIN LUMINA\n127.0.0.1 x.test\n").expect("hosts");
        let err = apply_hosts(&hosts_file, &[]).expect_err("broken");
        assert_eq!(err.exit_code(), exit::HOSTS_FILE);
        assert_eq!(
            std::fs::read_to_string(&hosts_file).expect("read"),
            "# BEGIN LUMINA\n127.0.0.1 x.test\n"
        );
    }
}
