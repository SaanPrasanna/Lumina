//! Per-service log files with simple size-based rotation.
//!
//! Child processes write straight into the file (no pipes), so a busy or stalled
//! log reader can never block a server, and worker processes spawned by the
//! server (e.g. php-cgi children) inherit the same file.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Rotate when a log grows beyond this many bytes.
pub const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;
/// Rotated files kept: `name.log.1` … `name.log.3`.
pub const KEEP_ROTATED: u32 = 3;

/// Opens `path` for appending, rotating it first if it is too large, and writes
/// a separator line so each run is easy to find.
pub fn open_for_run(path: &Path, header: &str) -> io::Result<File> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if std::fs::metadata(path).is_ok_and(|m| m.len() > MAX_LOG_BYTES) {
        rotate(path)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "\n===== {header} =====")?;
    Ok(file)
}

fn rotated(path: &Path, n: u32) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(format!(".{n}"));
    PathBuf::from(name)
}

fn rotate(path: &Path) -> io::Result<()> {
    let _ = std::fs::remove_file(rotated(path, KEEP_ROTATED));
    for n in (1..KEEP_ROTATED).rev() {
        let from = rotated(path, n);
        if from.exists() {
            std::fs::rename(&from, rotated(path, n + 1))?;
        }
    }
    std::fs::rename(path, rotated(path, 1))
}

/// The last `max_lines` lines of a log (reads at most the last 64 KiB).
pub fn tail(path: &Path, max_lines: usize) -> String {
    const WINDOW: u64 = 64 * 1024;
    let Ok(mut file) = File::open(path) else {
        return String::new();
    };
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    if file
        .seek(SeekFrom::Start(len.saturating_sub(WINDOW)))
        .is_err()
    {
        return String::new();
    }
    let mut bytes = Vec::new();
    if file.read_to_end(&mut bytes).is_err() {
        return String::new();
    }
    let text = String::from_utf8_lossy(&bytes);
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(max_lines)..].join("\n")
}

/// New text of a log since a previous read (see [`read_from`]).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Chunk {
    pub text: String,
    /// Where the next read starts.
    pub offset: u64,
    /// The file's size now.
    pub size: u64,
    /// The file was rotated or truncated: `text` starts a new file and the
    /// reader should drop what it showed before.
    pub reset: bool,
    /// Older text was skipped (more than `max` bytes were new).
    pub skipped: bool,
}

/// Reads what was appended to `path` since `offset` (`None`: the end of the
/// file, i.e. the last `max` bytes), at most `max` bytes, starting at a line
/// boundary when text is skipped. A missing file reads as empty.
pub fn read_from(path: &Path, offset: Option<u64>, max: u64) -> Chunk {
    let empty = |reset| Chunk {
        text: String::new(),
        offset: 0,
        size: 0,
        reset,
        skipped: false,
    };
    let Ok(mut file) = File::open(path) else {
        return empty(offset.is_some_and(|o| o > 0));
    };
    let size = file.metadata().map(|m| m.len()).unwrap_or(0);
    let reset = offset.is_some_and(|o| o > size);
    let from = match offset {
        Some(o) if o <= size => o,
        _ => size.saturating_sub(max),
    };
    let start = from.max(size.saturating_sub(max));
    let skipped = start > from || (offset.is_none() && start > 0) || (reset && start > 0);
    if file.seek(SeekFrom::Start(start)).is_err() {
        return empty(reset);
    }
    let mut bytes = Vec::with_capacity((size - start) as usize);
    if file.take(size - start).read_to_end(&mut bytes).is_err() {
        return empty(reset);
    }
    // Text read from the middle of the file starts after the next newline.
    let mut body = &bytes[..];
    if skipped && let Some(nl) = body.iter().position(|b| *b == b'\n') {
        body = &body[nl + 1..];
    }
    Chunk {
        text: String::from_utf8_lossy(body).into_owned(),
        offset: start + bytes.len() as u64,
        size,
        reset,
        skipped,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_only_what_was_appended() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("a.log");
        std::fs::write(&path, "one\ntwo\n").expect("write");
        let first = read_from(&path, None, 1024);
        assert_eq!(
            (
                first.text.as_str(),
                first.offset,
                first.reset,
                first.skipped
            ),
            ("one\ntwo\n", 8, false, false)
        );

        assert_eq!(read_from(&path, Some(first.offset), 1024).text, "");
        let mut f = OpenOptions::new().append(true).open(&path).expect("open");
        writeln!(f, "three").expect("append");
        let next = read_from(&path, Some(first.offset), 1024);
        assert_eq!((next.text.as_str(), next.offset), ("three\n", 14));
    }

    #[test]
    fn a_large_log_starts_at_a_line_and_rotation_resets() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("a.log");
        std::fs::write(&path, "aaaa\nbbbb\ncccc\n").expect("write");
        let tail = read_from(&path, None, 8);
        assert_eq!((tail.text.as_str(), tail.skipped), ("cccc\n", true));

        // Rotated: the file is shorter than where the reader stopped.
        std::fs::write(&path, "new\n").expect("rotate");
        let after = read_from(&path, Some(tail.offset), 1024);
        assert_eq!(
            (after.text.as_str(), after.reset, after.offset),
            ("new\n", true, 4)
        );

        let missing = read_from(&tmp.path().join("none.log"), Some(10), 1024);
        assert_eq!((missing.text.as_str(), missing.reset), ("", true));
    }

    #[test]
    fn rotates_large_logs_and_keeps_three() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("php.log");
        let big = vec![b'x'; (MAX_LOG_BYTES + 1) as usize];

        for _ in 0..5 {
            std::fs::write(&path, &big).expect("write");
            drop(open_for_run(&path, "run").expect("open"));
        }

        assert!(std::fs::metadata(&path).expect("current").len() < 100);
        for n in 1..=KEEP_ROTATED {
            assert!(rotated(&path, n).exists(), "missing .{n}");
        }
        assert!(!rotated(&path, KEEP_ROTATED + 1).exists());
    }

    #[test]
    fn tail_returns_last_lines() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("a.log");
        std::fs::write(&path, "1\n2\n3\n4\n").expect("write");
        assert_eq!(tail(&path, 2), "3\n4");
        assert_eq!(tail(&tmp.path().join("missing.log"), 2), "");
    }
}
