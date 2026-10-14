//! Types describing a supervised process and its lifecycle.

use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

use serde::Serialize;

/// Everything needed to (re)start a process.
#[derive(Debug, Clone)]
pub struct ProcessSpec {
    /// Unique ID within a supervisor, e.g. `"php-fcgi"`.
    pub id: String,
    pub program: PathBuf,
    pub args: Vec<OsString>,
    /// Added to (not replacing) the inherited environment.
    pub env: Vec<(OsString, OsString)>,
    pub cwd: Option<PathBuf>,
    /// stdout and stderr of the whole process tree are appended here.
    pub log_file: PathBuf,
    pub restart: RestartPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartPolicy {
    Never,
    /// Restart after an unexpected exit, waiting `backoff × attempt` each time.
    OnFailure {
        max_restarts: u32,
        backoff: Duration,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProcessState {
    Running,
    /// Exited unexpectedly; waiting to be restarted.
    Restarting,
    /// Exited unexpectedly and will not be restarted.
    Failed,
}

/// Snapshot of a supervised process.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessInfo {
    pub id: String,
    pub pid: u32,
    /// Unix timestamp (seconds) of the current run.
    pub started_at: u64,
    pub restarts: u32,
    pub state: ProcessState,
}

/// Lifecycle notifications, broadcast to subscribers (e.g. the UI).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessEvent {
    Started {
        id: String,
        pid: u32,
    },
    /// `expected` is true when the exit was requested through `stop`.
    Exited {
        id: String,
        code: Option<i32>,
        expected: bool,
    },
    Restarting {
        id: String,
        attempt: u32,
    },
    Failed {
        id: String,
        reason: String,
    },
}

impl ProcessEvent {
    pub fn id(&self) -> &str {
        match self {
            Self::Started { id, .. }
            | Self::Exited { id, .. }
            | Self::Restarting { id, .. }
            | Self::Failed { id, .. } => id,
        }
    }
}
