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

#[cfg(test)]
mod tests {
    use super::*;

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
