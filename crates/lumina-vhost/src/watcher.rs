//! Debounced filesystem watcher on the www root.
//!
//! `notify` watches the projects folder itself (not recursively: projects can
//! hold huge trees such as `node_modules`, and on Linux every watched folder
//! costs an inotify watch). A light periodic rescan catches what a top-level
//! watch cannot see, e.g. `public/` appearing inside an existing project after
//! `composer create-project`, and anything missed while `www` did not exist.
//! The callback runs only when the folder list or a docroot candidate changed.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use notify::{RecommendedWatcher, RecursiveMode, Watcher as _};

use crate::domain;

/// Quiet time after the last event before rescanning (a `git clone` or an
/// unzip produces bursts).
pub const DEBOUNCE: Duration = Duration::from_millis(500);
/// Upper bound on waiting for a burst to settle.
const MAX_DEBOUNCE: Duration = Duration::from_secs(2);
/// Default periodic rescan.
pub const POLL: Duration = Duration::from_secs(5);

enum Msg {
    Event,
    Stop,
}

/// Stops watching when dropped.
pub struct Watcher {
    tx: Sender<Msg>,
    thread: Option<JoinHandle<()>>,
}

impl Watcher {
    /// Watches `www`; `on_change` runs on the watcher's thread after the
    /// project folders changed.
    pub fn spawn(www: PathBuf, poll: Duration, on_change: impl Fn() + Send + 'static) -> Self {
        let (tx, rx) = mpsc::channel();
        let events = tx.clone();
        let thread = std::thread::Builder::new()
            .name("lumina-www-watcher".into())
            .spawn(move || {
                let mut notifier = None;
                let mut last = domain::list_folders(&www);
                loop {
                    if notifier.is_none() {
                        notifier = watch(&www, events.clone());
                    }
                    match rx.recv_timeout(poll) {
                        Ok(Msg::Stop) | Err(RecvTimeoutError::Disconnected) => break,
                        Ok(Msg::Event) => {
                            if settle(&rx) {
                                break;
                            }
                        }
                        Err(RecvTimeoutError::Timeout) => {}
                    }
                    let now = domain::list_folders(&www);
                    if now != last {
                        last = now;
                        on_change();
                    }
                    if !www.is_dir() {
                        // A removed and re-created folder needs a new watch.
                        notifier = None;
                    }
                }
            })
            .ok();
        if thread.is_none() {
            tracing::error!("could not start the projects folder watcher");
        }
        Self { tx, thread }
    }
}

/// Waits until no event arrived for [`DEBOUNCE`]. Returns `true` on stop.
fn settle(rx: &mpsc::Receiver<Msg>) -> bool {
    let started = Instant::now();
    loop {
        match rx.recv_timeout(DEBOUNCE) {
            Ok(Msg::Event) if started.elapsed() < MAX_DEBOUNCE => continue,
            Ok(Msg::Event) | Err(RecvTimeoutError::Timeout) => return false,
            Ok(Msg::Stop) | Err(RecvTimeoutError::Disconnected) => return true,
        }
    }
}

fn watch(www: &Path, tx: Sender<Msg>) -> Option<RecommendedWatcher> {
    if !www.is_dir() {
        return None;
    }
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if res.is_ok_and(|e| !e.kind.is_access()) {
            let _ = tx.send(Msg::Event);
        }
    })
    .map_err(|e| tracing::warn!("cannot watch {}: {e}", www.display()))
    .ok()?;
    watcher
        .watch(www, RecursiveMode::NonRecursive)
        .map_err(|e| tracing::warn!("cannot watch {}: {e}", www.display()))
        .ok()?;
    Some(watcher)
}

impl Drop for Watcher {
    fn drop(&mut self) {
        let _ = self.tx.send(Msg::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    fn wait_for(count: &AtomicUsize, at_least: usize, within: Duration) -> bool {
        let deadline = Instant::now() + within;
        while Instant::now() < deadline {
            if count.load(Ordering::SeqCst) >= at_least {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }

    #[test]
    fn new_and_removed_folders_trigger_within_3s() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let www = tmp.path().join("www");
        std::fs::create_dir_all(&www).expect("mkdir");
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        // A long poll interval, so the notify path is what is being tested.
        let watcher = Watcher::spawn(www.clone(), Duration::from_secs(60), move || {
            c.fetch_add(1, Ordering::SeqCst);
        });
        std::thread::sleep(Duration::from_millis(200));

        std::fs::create_dir(www.join("blog")).expect("mkdir blog");
        assert!(
            wait_for(&count, 1, Duration::from_secs(3)),
            "creation not seen"
        );

        // Files and dot-folders do not count.
        std::fs::write(www.join("notes.txt"), "x").expect("file");
        std::fs::create_dir(www.join(".cache")).expect("dot dir");
        std::thread::sleep(Duration::from_millis(1200));
        assert_eq!(count.load(Ordering::SeqCst), 1);

        std::fs::remove_dir(www.join("blog")).expect("rmdir");
        assert!(
            wait_for(&count, 2, Duration::from_secs(3)),
            "removal not seen"
        );
        drop(watcher);
    }

    #[test]
    fn polling_sees_docroots_appearing_inside_projects() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let www = tmp.path().to_path_buf();
        std::fs::create_dir_all(www.join("app")).expect("mkdir");
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        let _watcher = Watcher::spawn(www.clone(), Duration::from_millis(200), move || {
            c.fetch_add(1, Ordering::SeqCst);
        });
        std::thread::sleep(Duration::from_millis(300));
        std::fs::create_dir(www.join("app").join("public")).expect("public");
        assert!(wait_for(&count, 1, Duration::from_secs(3)));
    }
}
