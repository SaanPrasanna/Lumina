//! Keeps the hosts file's Lumina block equal to the desired sites.
//!
//! Reading the hosts file needs no privileges, so the block is compared first
//! and the helper is only run (one OS prompt) when something differs. A
//! declined prompt is remembered: the same change is not asked for again until
//! the user retries or the sites change.

use std::net::{IpAddr, Ipv4Addr};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use lumina_helper::exit;
use lumina_platform::elevation::{self, ElevationError};
use lumina_platform::hosts::{self, Entry};
use serde::Serialize;

use crate::sites::Site;

/// The hosts block for `sites`: `127.0.0.1 <domain>` each, sorted.
///
/// IPv4 only on purpose: the web servers listen on 127.0.0.1, and on Windows a
/// `::1` entry with nothing listening there makes every request wait for the
/// refused IPv6 connection first.
pub fn desired_entries(sites: &[Site]) -> Vec<Entry> {
    let mut entries: Vec<Entry> = sites
        .iter()
        .map(|s| Entry::new(IpAddr::V4(Ipv4Addr::LOCALHOST), s.domain.clone()))
        .collect();
    entries.sort();
    entries.dedup();
    entries
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    /// The block already matched; nothing was run.
    UpToDate,
    /// The helper rewrote the block.
    Updated,
    /// The administrator prompt was dismissed.
    Declined,
    /// `lumina-helper` is not next to the app.
    HelperMissing,
    Failed,
}

/// The result of the last [`HostsSync::ensure`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Attempt {
    pub outcome: Outcome,
    /// Why it failed, in English (for the UI and the log).
    pub message: Option<String>,
}

impl Attempt {
    fn new(outcome: Outcome) -> Self {
        Self {
            outcome,
            message: None,
        }
    }

    fn failed(message: impl Into<String>) -> Self {
        Self {
            outcome: Outcome::Failed,
            message: Some(message.into()),
        }
    }
}

/// What the Sites page shows about the hosts file.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostsReport {
    pub file: PathBuf,
    /// The block has exactly the desired entries.
    pub in_sync: bool,
    /// Desired domains that the block lacks.
    pub missing: Vec<String>,
    /// Domains in the block that are no longer wanted.
    pub extra: Vec<String>,
    /// An administrator prompt is open right now.
    pub pending: bool,
    pub last: Option<Attempt>,
    /// The hosts file could not be read or its block is broken.
    pub error: Option<String>,
}

#[derive(Default)]
struct State {
    pending: bool,
    /// The entries whose prompt was declined.
    declined: Option<Vec<Entry>>,
    last: Option<Attempt>,
}

pub struct HostsSync {
    helper: PathBuf,
    hosts_file: PathBuf,
    /// Where the request file is written (inside the data root).
    work_dir: PathBuf,
    state: Mutex<State>,
    /// One helper run at a time.
    run: tokio::sync::Mutex<()>,
}

impl HostsSync {
    pub fn new(helper: PathBuf, hosts_file: PathBuf, work_dir: PathBuf) -> Self {
        Self {
            helper,
            hosts_file,
            work_dir,
            state: Mutex::new(State::default()),
            run: tokio::sync::Mutex::new(()),
        }
    }

    pub fn hosts_file(&self) -> &Path {
        &self.hosts_file
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn current(&self) -> Result<Vec<Entry>, String> {
        let raw = std::fs::read(&self.hosts_file)
            .map_err(|e| format!("{}: {e}", self.hosts_file.display()))?;
        let mut entries = hosts::parse_block(&raw).map_err(|e| e.to_string())?;
        entries.sort();
        entries.dedup();
        Ok(entries)
    }

    pub fn report(&self, desired: &[Entry]) -> HostsReport {
        let state = self.state();
        let (current, error) = match self.current() {
            Ok(c) => (c, None),
            Err(e) => (Vec::new(), Some(e)),
        };
        let names = |from: &[Entry], not_in: &[Entry]| {
            from.iter()
                .filter(|e| !not_in.contains(e))
                .map(|e| e.host.clone())
                .collect::<Vec<_>>()
        };
        HostsReport {
            file: self.hosts_file.clone(),
            in_sync: error.is_none() && current == desired,
            missing: names(desired, &current),
            extra: names(&current, desired),
            pending: state.pending,
            last: state.last.clone(),
            error,
        }
    }

    /// Makes the block equal to `desired` (sorted, as from [`desired_entries`]),
    /// prompting for administrator rights only if it differs. Without `force`,
    /// a change the user already declined is not asked for again.
    pub async fn ensure(&self, desired: Vec<Entry>, force: bool) -> Attempt {
        let _run = self.run.lock().await;
        let attempt = self.ensure_locked(desired, force).await;
        if attempt.outcome != Outcome::UpToDate || self.state().last.is_none() {
            match &attempt.message {
                Some(msg) => tracing::warn!("hosts file: {:?}: {msg}", attempt.outcome),
                None => tracing::info!("hosts file: {:?}", attempt.outcome),
            }
        }
        self.state().last = Some(attempt.clone());
        attempt
    }

    async fn ensure_locked(&self, desired: Vec<Entry>, force: bool) -> Attempt {
        match self.current() {
            Ok(current) if current == desired => {
                self.state().declined = None;
                return Attempt::new(Outcome::UpToDate);
            }
            Ok(_) => {}
            Err(e) => return Attempt::failed(e),
        }
        if !force && self.state().declined.as_ref() == Some(&desired) {
            return Attempt::new(Outcome::Declined);
        }
        if !self.helper.is_file() {
            return Attempt {
                outcome: Outcome::HelperMissing,
                message: Some(format!("{} was not found", self.helper.display())),
            };
        }

        let request = self.work_dir.join("hosts-request.json");
        let body = serde_json::json!({
            "entries": desired
                .iter()
                .map(|e| serde_json::json!({ "ip": e.ip.to_string(), "host": e.host }))
                .collect::<Vec<_>>(),
        });
        if let Err(e) = lumina_core::fs::write_atomic(&request, body.to_string().as_bytes()) {
            return Attempt::failed(e.to_string());
        }

        self.state().pending = true;
        let helper = self.helper.clone();
        let request_arg = request.clone();
        let result = tokio::task::spawn_blocking(move || {
            elevation::run_elevated(&helper, &["hosts-apply".as_ref(), request_arg.as_os_str()])
        })
        .await;
        self.state().pending = false;
        let _ = std::fs::remove_file(&request);

        let exit = match result {
            Ok(Ok(exit)) => exit,
            Ok(Err(ElevationError::Declined)) => {
                self.state().declined = Some(desired);
                return Attempt::new(Outcome::Declined);
            }
            Ok(Err(e)) => return Attempt::failed(e.to_string()),
            Err(e) => return Attempt::failed(format!("helper task failed: {e}")),
        };
        if exit.code != exit::OK {
            let what = match exit.code {
                exit::INVALID => "the helper rejected the request",
                exit::HOSTS_FILE => {
                    "the hosts file has a broken Lumina block or an unknown encoding; fix or remove the block by hand"
                }
                exit::IO => {
                    "the helper could not write the hosts file (is it locked by antivirus?)"
                }
                exit::USAGE => "the helper was called incorrectly",
                _ => "the helper failed",
            };
            let detail = if exit.stderr.is_empty() {
                format!("{what} (exit code {})", exit.code)
            } else {
                format!("{what}: {}", exit.stderr)
            };
            return Attempt::failed(detail);
        }
        self.state().declined = None;
        match self.current() {
            Ok(current) if current == desired => Attempt::new(Outcome::Updated),
            Ok(_) => Attempt::failed(
                "the helper finished but the hosts file does not have the new entries",
            ),
            Err(e) => Attempt::failed(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sites::DocrootSource;

    fn site(domain: &str) -> Site {
        Site {
            name: domain.into(),
            domain: domain.into(),
            path: PathBuf::new(),
            docroot: PathBuf::new(),
            docroot_source: DocrootSource::Detected(None),
            docroot_exists: true,
        }
    }

    #[test]
    fn desired_entries_are_sorted_ipv4() {
        let entries = desired_entries(&[site("b.test"), site("a.test")]);
        assert_eq!(
            entries
                .iter()
                .map(|e| (e.ip.to_string(), e.host.as_str()))
                .collect::<Vec<_>>(),
            [
                ("127.0.0.1".to_owned(), "a.test"),
                ("127.0.0.1".to_owned(), "b.test")
            ]
        );
    }

    #[tokio::test]
    async fn no_prompt_when_in_sync_and_report_lists_differences() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let hosts_file = tmp.path().join("hosts");
        let desired = desired_entries(&[site("a.test")]);
        std::fs::write(
            &hosts_file,
            hosts::apply_block(b"", &desired).expect("block"),
        )
        .expect("write");
        // The helper does not exist: any attempt to run it would say so.
        let sync = HostsSync::new(tmp.path().join("no-helper"), hosts_file, tmp.path().into());

        assert_eq!(
            sync.ensure(desired.clone(), true).await.outcome,
            Outcome::UpToDate
        );
        assert!(sync.report(&desired).in_sync);

        let more = desired_entries(&[site("a.test"), site("b.test")]);
        let report = sync.report(&more);
        assert!(!report.in_sync);
        assert_eq!(report.missing, ["b.test"]);
        assert!(report.extra.is_empty());
        assert_eq!(
            sync.ensure(more, false).await.outcome,
            Outcome::HelperMissing
        );
    }

    #[tokio::test]
    async fn broken_hosts_block_is_reported_without_prompting() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let hosts_file = tmp.path().join("hosts");
        std::fs::write(&hosts_file, "# BEGIN LUMINA\n").expect("write");
        let sync = HostsSync::new(tmp.path().join("no-helper"), hosts_file, tmp.path().into());
        let attempt = sync.ensure(Vec::new(), true).await;
        assert_eq!(attempt.outcome, Outcome::Failed);
        assert!(sync.report(&[]).error.is_some());
    }
}
