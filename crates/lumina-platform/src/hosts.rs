//! Safe, marker-delimited edits of the hosts file.
//!
//! Lumina owns exactly one block and nothing else:
//!
//! ```text
//! # BEGIN LUMINA - managed by Lumina, do not edit inside this block
//! 127.0.0.1  my-app.test
//! # END LUMINA
//! ```
//!
//! Edits work on bytes, so everything outside the block (other tools' entries,
//! comments in any 8-bit encoding, a UTF-8 BOM, mixed line endings) is kept
//! byte for byte. UTF-16 files (with a BOM) are decoded and re-encoded.
//! New lines use the file's own line ending.
//!
//! Reading needs no privileges; writing is done by `lumina-helper`.

use std::fmt;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::command::hide_console;

pub const BEGIN_MARKER: &str = "# BEGIN LUMINA";
pub const END_MARKER: &str = "# END LUMINA";
const BEGIN_LINE: &str = "# BEGIN LUMINA - managed by Lumina, do not edit inside this block";

/// Longest host name a hosts file entry may carry (DNS limit).
pub const MAX_HOST_LEN: usize = 253;
const MAX_LABEL_LEN: usize = 63;

/// One `ip host` line inside the block.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Entry {
    pub ip: IpAddr,
    pub host: String,
}

impl Entry {
    pub fn new(ip: IpAddr, host: impl Into<String>) -> Self {
        Self {
            ip,
            host: host.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostsError {
    /// `# BEGIN LUMINA` without `# END LUMINA`, the other way round, or two blocks.
    /// Lumina refuses to guess which lines are its own.
    BrokenBlock(String),
    /// The file is UTF-16 but not valid UTF-16, or uses an unsupported encoding.
    Encoding(String),
}

impl fmt::Display for HostsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BrokenBlock(why) => {
                write!(f, "the Lumina block in the hosts file is broken: {why}")
            }
            Self::Encoding(why) => write!(f, "the hosts file cannot be read: {why}"),
        }
    }
}

impl std::error::Error for HostsError {}

/// The system hosts file.
pub fn system_hosts_path() -> PathBuf {
    #[cfg(windows)]
    {
        let root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
        PathBuf::from(root).join(r"System32\drivers\etc\hosts")
    }
    #[cfg(not(windows))]
    {
        PathBuf::from("/etc/hosts")
    }
}

/// Lowercase DNS-style names only: `^[a-z0-9-]+(\.[a-z0-9-]+)*$`, at most 253
/// characters, labels of 1-63 characters that neither start nor end with `-`.
pub fn is_valid_host(host: &str) -> bool {
    if host.is_empty() || host.len() > MAX_HOST_LEN {
        return false;
    }
    host.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= MAX_LABEL_LEN
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    })
}

/// Only loopback addresses may be written: a hosts entry can never send a
/// name to another machine.
pub fn is_allowed_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.octets() == [127, 0, 0, 1],
        IpAddr::V6(v6) => v6.is_loopback(),
    }
}

/// Text encoding of a hosts file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Encoding {
    /// UTF-8 or any ASCII-compatible 8-bit encoding; edited as bytes.
    Bytes,
    Utf16Le,
}

fn decode(raw: &[u8]) -> Result<(Encoding, Vec<u8>), HostsError> {
    match raw {
        [0xFF, 0xFE, rest @ ..] => {
            if rest.len() % 2 != 0 {
                return Err(HostsError::Encoding(
                    "odd number of bytes in a UTF-16 file".into(),
                ));
            }
            let units: Vec<u16> = rest
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_le_bytes(*c))
                .collect();
            let text = String::from_utf16(&units)
                .map_err(|_| HostsError::Encoding("invalid UTF-16".into()))?;
            Ok((Encoding::Utf16Le, text.into_bytes()))
        }
        [0xFE, 0xFF, ..] => Err(HostsError::Encoding(
            "UTF-16 big-endian is not supported".into(),
        )),
        _ => Ok((Encoding::Bytes, raw.to_vec())),
    }
}

fn encode(encoding: Encoding, bytes: Vec<u8>) -> Vec<u8> {
    match encoding {
        Encoding::Bytes => bytes,
        Encoding::Utf16Le => {
            // Only reached for content that came from a valid String plus ASCII.
            let text = String::from_utf8_lossy(&bytes);
            let mut out = vec![0xFF, 0xFE];
            for unit in text.encode_utf16() {
                out.extend_from_slice(&unit.to_le_bytes());
            }
            out
        }
    }
}

/// Lines including their terminators; the last one may have none.
fn lines(bytes: &[u8]) -> Vec<&[u8]> {
    bytes.split_inclusive(|&b| b == b'\n').collect()
}

fn line_text(line: &[u8]) -> &[u8] {
    let line = line.strip_suffix(b"\n").unwrap_or(line);
    line.strip_suffix(b"\r").unwrap_or(line)
}

fn trimmed(line: &[u8]) -> &[u8] {
    line_text(line).trim_ascii()
}

/// Line indexes of BEGIN and END, if there is a block.
fn find_block(lines: &[&[u8]]) -> Result<Option<(usize, usize)>, HostsError> {
    let mut begin = None;
    let mut found = None;
    for (i, line) in lines.iter().enumerate() {
        let t = trimmed(line);
        if t.starts_with(BEGIN_MARKER.as_bytes()) {
            if begin.is_some() || found.is_some() {
                return Err(HostsError::BrokenBlock(format!(
                    "a second \"{BEGIN_MARKER}\" on line {}",
                    i + 1
                )));
            }
            begin = Some(i);
        } else if t.starts_with(END_MARKER.as_bytes()) {
            match begin.take() {
                Some(b) => found = Some((b, i)),
                None => {
                    return Err(HostsError::BrokenBlock(format!(
                        "\"{END_MARKER}\" without \"{BEGIN_MARKER}\" on line {}",
                        i + 1
                    )));
                }
            }
        }
    }
    if let Some(b) = begin {
        return Err(HostsError::BrokenBlock(format!(
            "\"{BEGIN_MARKER}\" on line {} has no \"{END_MARKER}\"",
            b + 1
        )));
    }
    Ok(found)
}

/// The entries currently inside the block (empty without a block). Lines that
/// are not `ip host...` are ignored.
pub fn parse_block(raw: &[u8]) -> Result<Vec<Entry>, HostsError> {
    let (_, bytes) = decode(raw)?;
    let lines = lines(&bytes);
    let Some((begin, end)) = find_block(&lines)? else {
        return Ok(Vec::new());
    };
    let mut entries = Vec::new();
    for line in &lines[begin + 1..end] {
        let text = String::from_utf8_lossy(trimmed(line));
        let text = text.split('#').next().unwrap_or_default();
        let mut cols = text.split_whitespace();
        let Some(ip) = cols.next().and_then(|ip| ip.parse::<IpAddr>().ok()) else {
            continue;
        };
        for host in cols {
            entries.push(Entry::new(ip, host.to_owned()));
        }
    }
    Ok(entries)
}

/// The line ending this file uses: CRLF if any line has one, else LF, else
/// (empty or single-line file) the platform's.
fn line_ending(bytes: &[u8]) -> &'static [u8] {
    if let Some(pos) = bytes.iter().position(|&b| b == b'\n') {
        if pos > 0 && bytes[pos - 1] == b'\r' {
            return b"\r\n";
        }
        return b"\n";
    }
    if cfg!(windows) { b"\r\n" } else { b"\n" }
}

fn render_block(entries: &[Entry], eol: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(BEGIN_LINE.as_bytes());
    out.extend_from_slice(eol);
    for e in entries {
        out.extend_from_slice(format!("{:<10} {}", e.ip, e.host).as_bytes());
        out.extend_from_slice(eol);
    }
    out.extend_from_slice(END_MARKER.as_bytes());
    out.extend_from_slice(eol);
    out
}

/// The hosts file with its Lumina block replaced by `entries`: the block is
/// updated in place, appended at the end if there is none, and removed when
/// `entries` is empty. Everything outside the block is returned unchanged.
pub fn apply_block(raw: &[u8], entries: &[Entry]) -> Result<Vec<u8>, HostsError> {
    let (encoding, bytes) = decode(raw)?;
    let eol = line_ending(&bytes);
    let all = lines(&bytes);
    let block = (!entries.is_empty()).then(|| render_block(entries, eol));

    let mut out = Vec::with_capacity(bytes.len() + 256);
    match find_block(&all)? {
        Some((begin, end)) => {
            for line in &all[..begin] {
                out.extend_from_slice(line);
            }
            if let Some(block) = &block {
                out.extend_from_slice(block);
            }
            for line in &all[end + 1..] {
                out.extend_from_slice(line);
            }
        }
        None => {
            out.extend_from_slice(&bytes);
            if let Some(block) = &block {
                if !out.is_empty() && !out.ends_with(b"\n") {
                    out.extend_from_slice(eol);
                }
                out.extend_from_slice(block);
            }
        }
    }
    Ok(encode(encoding, out))
}

/// Everything outside the block, for tests and sanity checks.
pub fn outside_block(raw: &[u8]) -> Result<Vec<u8>, HostsError> {
    apply_block(raw, &[])
}

/// Writes new hosts content in place (keeping the file's permissions and
/// owner, which a rename-over would replace) after copying the current file to
/// `hosts.lumina.bak` next to it. On Windows a read-only file is made writable
/// for the write and read-only again afterwards (on Unix root writes anyway, and
/// clearing the flag there would make the file world-writable).
pub fn write_hosts(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    // Written rather than copied: a copy would carry a read-only attribute over
    // and block the next backup.
    let backup = path.with_file_name("hosts.lumina.bak");
    std::fs::write(&backup, std::fs::read(path)?)?;

    let mut perms = std::fs::metadata(path)?.permissions();
    let was_readonly = cfg!(windows) && perms.readonly();
    if was_readonly {
        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(false);
        std::fs::set_permissions(path, perms.clone())?;
    }
    let result = std::fs::write(path, contents);
    if was_readonly {
        perms.set_readonly(true);
        let _ = std::fs::set_permissions(path, perms);
    }
    result?;

    if std::fs::read(path)? != contents {
        return Err(std::io::Error::other(
            "the hosts file changed while it was being written",
        ));
    }
    Ok(())
}

/// Asks the OS resolver to forget cached names so new entries apply at once.
/// Best effort.
pub fn flush_dns() {
    let commands: &[(&str, &[&str])] = if cfg!(windows) {
        &[("ipconfig", &["/flushdns"])]
    } else if cfg!(target_os = "macos") {
        &[
            ("dscacheutil", &["-flushcache"]),
            ("killall", &["-HUP", "mDNSResponder"]),
        ]
    } else {
        &[("resolvectl", &["flush-caches"])]
    };
    for (program, args) in commands {
        let mut cmd = Command::new(program);
        cmd.args(*args);
        hide_console(&mut cmd);
        let _ = cmd.output();
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, Ipv6Addr};

    use super::*;

    fn v4(host: &str) -> Entry {
        Entry::new(IpAddr::V4(Ipv4Addr::LOCALHOST), host)
    }

    const ORIGINAL: &[u8] =
        b"# Copyright (c) Microsoft\r\n#\r\n127.0.0.1      shop.test   #laragon magic!   \r\n\r\n";

    #[test]
    fn appends_block_with_the_files_line_ending() {
        let out = apply_block(ORIGINAL, &[v4("blog.test")]).expect("apply");
        let text = String::from_utf8(out.clone()).expect("utf8");
        assert!(text.starts_with(std::str::from_utf8(ORIGINAL).expect("utf8")));
        assert!(text.ends_with(
            "# BEGIN LUMINA - managed by Lumina, do not edit inside this block\r\n127.0.0.1  blog.test\r\n# END LUMINA\r\n"
        ), "{text:?}");
        assert_eq!(parse_block(&out).expect("parse"), vec![v4("blog.test")]);
    }

    #[test]
    fn updates_in_place_and_removes_cleanly() {
        let with_block = apply_block(ORIGINAL, &[v4("a.test")]).expect("add");
        let mut tail = with_block.clone();
        tail.extend_from_slice(b"10.0.0.1 after.example\r\n");

        let updated = apply_block(&tail, &[v4("a.test"), v4("b.test")]).expect("update");
        assert!(updated.ends_with(b"# END LUMINA\r\n10.0.0.1 after.example\r\n"));
        assert_eq!(parse_block(&updated).expect("parse").len(), 2);

        let removed = apply_block(&updated, &[]).expect("remove");
        let mut expected = ORIGINAL.to_vec();
        expected.extend_from_slice(b"10.0.0.1 after.example\r\n");
        assert_eq!(removed, expected);
    }

    #[test]
    fn lf_files_and_missing_final_newline() {
        let out = apply_block(b"# hosts\n127.0.0.1 localhost", &[v4("x.test")]).expect("apply");
        assert_eq!(
            out,
            b"# hosts\n127.0.0.1 localhost\n# BEGIN LUMINA - managed by Lumina, do not edit inside this block\n127.0.0.1  x.test\n# END LUMINA\n"
        );
    }

    #[test]
    fn empty_file_and_no_change_without_entries() {
        assert_eq!(apply_block(b"", &[]).expect("noop"), b"");
        assert_eq!(apply_block(ORIGINAL, &[]).expect("noop"), ORIGINAL);
        let out = apply_block(b"", &[v4("x.test")]).expect("apply");
        assert!(out.starts_with(BEGIN_MARKER.as_bytes()));
    }

    #[test]
    fn non_utf8_bytes_and_bom_outside_the_block_survive() {
        let mut raw = vec![0xEF, 0xBB, 0xBF];
        raw.extend_from_slice(b"# caf\xE9 (latin-1)\r\n127.0.0.1 a\r\n");
        let out = apply_block(&raw, &[v4("x.test")]).expect("apply");
        assert!(out.starts_with(&raw));
        assert_eq!(outside_block(&out).expect("outside"), raw);
    }

    #[test]
    fn utf16_files_are_decoded_and_re_encoded() {
        let text = "# h\u{00e9}llo\r\n127.0.0.1 localhost\r\n";
        let mut raw = vec![0xFF, 0xFE];
        for u in text.encode_utf16() {
            raw.extend_from_slice(&u.to_le_bytes());
        }
        let out = apply_block(&raw, &[v4("x.test")]).expect("apply");
        assert_eq!(&out[..2], &[0xFF, 0xFE]);
        assert_eq!(parse_block(&out).expect("parse"), vec![v4("x.test")]);
        assert_eq!(apply_block(&out, &[]).expect("remove"), raw);
    }

    #[test]
    fn broken_blocks_are_refused() {
        for raw in [
            &b"# BEGIN LUMINA\n127.0.0.1 a.test\n"[..],
            b"# END LUMINA\n",
            b"# BEGIN LUMINA\n# END LUMINA\n# BEGIN LUMINA\n# END LUMINA\n",
            b"# BEGIN LUMINA\n# BEGIN LUMINA\n# END LUMINA\n",
        ] {
            assert!(
                matches!(
                    apply_block(raw, &[v4("a.test")]),
                    Err(HostsError::BrokenBlock(_))
                ),
                "{:?}",
                String::from_utf8_lossy(raw)
            );
        }
    }

    #[test]
    fn outside_is_byte_identical_after_100_cycles() {
        let mut raw = ORIGINAL.to_vec();
        raw.extend_from_slice(b"\t# tabbed\n\n0.0.0.0 ads.example # trailing  \r\n");
        let outside = raw.clone();
        let mut current = raw;
        for cycle in 0..100usize {
            let entries: Vec<Entry> = (0..cycle % 7)
                .map(|i| v4(&format!("site{}.test", (i * 31 + cycle) % 50)))
                .chain(
                    (cycle % 3 == 0)
                        .then(|| Entry::new(IpAddr::V6(Ipv6Addr::LOCALHOST), "v6.test")),
                )
                .collect();
            current = apply_block(&current, &entries).expect("apply");
            assert_eq!(
                parse_block(&current).expect("parse"),
                entries,
                "cycle {cycle}"
            );
            assert_eq!(
                outside_block(&current).expect("outside"),
                outside,
                "cycle {cycle}"
            );
        }
    }

    #[test]
    fn host_validation() {
        for ok in ["blog.test", "my-app.test", "a", "1.2.test", "x-1.y-2.local"] {
            assert!(is_valid_host(ok), "{ok}");
        }
        for bad in [
            "",
            "Blog.test",
            "-a.test",
            "a-.test",
            "a..test",
            ".test",
            "a.test.",
            "a b",
            "a_b.test",
            "a\n127.0.0.1 evil",
            "a#b",
            "ünï.test",
        ] {
            assert!(!is_valid_host(bad), "{bad:?}");
        }
        assert!(!is_valid_host(&"a".repeat(64)));
        assert!(is_valid_host(&"a".repeat(63)));
        assert!(!is_valid_host(&format!(
            "{}.test",
            vec!["a".repeat(60); 5].join(".")
        )));
    }

    #[test]
    fn only_loopback_ips() {
        assert!(is_allowed_ip(&"127.0.0.1".parse().expect("ip")));
        assert!(is_allowed_ip(&"::1".parse().expect("ip")));
        for bad in ["127.0.0.2", "0.0.0.0", "10.0.0.1", "::", "8.8.8.8"] {
            assert!(!is_allowed_ip(&bad.parse().expect("ip")), "{bad}");
        }
    }

    #[cfg(windows)]
    #[test]
    fn write_hosts_keeps_a_backup_and_handles_read_only() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("hosts");
        std::fs::write(&path, b"old\n").expect("write");
        let mut perms = std::fs::metadata(&path).expect("meta").permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(&path, perms).expect("readonly");

        write_hosts(&path, b"new\n").expect("write_hosts");

        assert_eq!(std::fs::read(&path).expect("read"), b"new\n");
        assert_eq!(
            std::fs::read(tmp.path().join("hosts.lumina.bak")).expect("bak"),
            b"old\n"
        );
        let mut perms = std::fs::metadata(&path).expect("meta").permissions();
        assert!(perms.readonly());

        // A second write must not trip over the first backup.
        write_hosts(&path, b"newer\n").expect("again");
        assert_eq!(
            std::fs::read(tmp.path().join("hosts.lumina.bak")).expect("bak"),
            b"new\n"
        );

        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(false);
        std::fs::set_permissions(&path, perms).expect("cleanup");
    }
}
