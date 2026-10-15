//! PID ledger (`<data-root>/tmp/pids.json`): every process Lumina starts is
//! recorded here and removed when it is stopped. Entries left behind mean Lumina
//! did not shut down cleanly (crash, power loss, kill from Task Manager); on the
//! next start [`Ledger::sweep`] kills those leftovers.
//!
//! Safety: PIDs get reused, so a leftover is killed only if its executable lives
//! under the directory Lumina installs runtimes into (`<data-root>/bin`). A user's
//! own `php artisan serve` started from a terminal is never in the ledger.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
struct Entry {
    id: String,
    pid: u32,
    exe: PathBuf,
}

pub struct Ledger {
    path: PathBuf,
    entries: Mutex<Vec<Entry>>,
}

/// A leftover process killed by [`Ledger::sweep`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SweptProcess {
    pub id: String,
    pub pid: u32,
    pub exe: PathBuf,
}

impl Ledger {
    /// Loads the ledger at `path`. A missing or corrupt file starts empty.
    pub fn open(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let entries = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            path,
            entries: Mutex::new(entries),
        }
    }

    /// An in-memory ledger for tests and for running before a data root exists.
    pub fn disabled() -> Self {
        Self {
            path: PathBuf::new(),
            entries: Mutex::new(Vec::new()),
        }
    }

    pub fn record(&self, id: &str, pid: u32, exe: &Path) {
        let mut entries = self.lock();
        entries.retain(|e| e.pid != pid);
        entries.push(Entry {
            id: id.to_owned(),
            pid,
            exe: exe.to_path_buf(),
        });
        self.persist(&entries);
    }

    pub fn remove(&self, pid: u32) {
        let mut entries = self.lock();
        let before = entries.len();
        entries.retain(|e| e.pid != pid);
        if entries.len() != before {
            self.persist(&entries);
        }
    }

    /// Kills processes left over from a previous run whose executable is inside
    /// `owned_root`, then clears the ledger. Call once at startup, before any
    /// service is started.
    pub fn sweep(&self, owned_root: &Path) -> Vec<SweptProcess> {
        let mut entries = self.lock();
        let mut swept = Vec::new();
        if entries.is_empty() {
            return swept;
        }

        let mut sys = System::new();
        for entry in entries.iter() {
            let candidates = lumina_platform::process_group::orphaned_members(entry.pid)
                .unwrap_or_else(|_| vec![entry.pid]);
            let pids: Vec<Pid> = candidates.iter().map(|&p| Pid::from_u32(p)).collect();
            sys.refresh_processes_specifics(
                ProcessesToUpdate::Some(&pids),
                true,
                ProcessRefreshKind::nothing().with_exe(UpdateKind::Always),
            );
            let ours = pids.iter().any(|pid| {
                sys.process(*pid)
                    .and_then(|p| p.exe())
                    .is_some_and(|exe| is_within(exe, owned_root))
            });
            if !ours {
                continue;
            }
            tracing::warn!(
                "killing leftover `{}` (pid {}) from a previous run",
                entry.id,
                entry.pid
            );
            if let Err(err) = lumina_platform::process_group::kill_orphaned_group(entry.pid) {
                tracing::error!("could not kill leftover pid {}: {err}", entry.pid);
            }
            swept.push(SweptProcess {
                id: entry.id.clone(),
                pid: entry.pid,
                exe: entry.exe.clone(),
            });
        }

        entries.clear();
        self.persist(&entries);
        swept
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Entry>> {
        self.entries.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn persist(&self, entries: &[Entry]) {
        if self.path.as_os_str().is_empty() {
            return;
        }
        let result = serde_json::to_vec_pretty(entries)
            .map_err(|e| lumina_core::Error::Other(e.to_string()))
            .and_then(|bytes| lumina_core::fs::write_atomic(&self.path, &bytes));
        if let Err(err) = result {
            tracing::error!("could not write PID ledger: {err}");
        }
    }
}

/// Path containment that ignores case on Windows (paths there are case-insensitive).
fn is_within(path: &Path, root: &Path) -> bool {
    if cfg!(windows) {
        let path = path.to_string_lossy().to_lowercase().replace('/', "\\");
        let root = root.to_string_lossy().to_lowercase().replace('/', "\\");
        let root = root.trim_end_matches('\\');
        path.starts_with(&format!("{root}\\"))
    } else {
        path.starts_with(root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_persist_and_reload() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("pids.json");

        let ledger = Ledger::open(&path);
        ledger.record("php-fcgi", 111, Path::new("/x/php-cgi"));
        ledger.record("nginx", 222, Path::new("/x/nginx"));
        ledger.remove(111);

        let reloaded = Ledger::open(&path);
        let entries = reloaded.lock();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "nginx");
    }

    #[test]
    fn corrupt_file_starts_empty() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("pids.json");
        std::fs::write(&path, "{not json").expect("write");
        assert!(Ledger::open(&path).lock().is_empty());
    }

    #[test]
    fn sweep_never_kills_processes_outside_the_owned_root() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let ledger = Ledger::open(tmp.path().join("pids.json"));
        // This test process is alive, but its exe is not under `bin/`.
        ledger.record("imposter", std::process::id(), Path::new("/whatever"));

        let swept = ledger.sweep(&tmp.path().join("bin"));

        assert!(swept.is_empty());
        assert!(ledger.lock().is_empty(), "ledger cleared after sweep");
    }

    #[test]
    fn containment_check() {
        if cfg!(windows) {
            assert!(is_within(
                Path::new(r"C:\Lumina\bin\php\8.4\php-cgi.exe"),
                Path::new(r"c:\lumina\BIN")
            ));
            assert!(!is_within(
                Path::new(r"C:\Lumina\bin2\php.exe"),
                Path::new(r"C:\Lumina\bin")
            ));
        } else {
            assert!(is_within(
                Path::new("/home/a/Lumina/bin/php"),
                Path::new("/home/a/Lumina/bin")
            ));
            assert!(!is_within(
                Path::new("/home/a/Lumina/bin2/php"),
                Path::new("/home/a/Lumina/bin")
            ));
        }
    }
}
