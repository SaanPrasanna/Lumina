//! Concrete service drivers built on `lumina-process`, plus the orchestrator that
//! starts/stops them as a stack and switches profiles.

pub mod apache;
pub mod apps;
pub mod database;
pub mod extra;
pub mod install;
pub mod mailpit;
pub mod nginx;
pub mod orchestrator;
pub mod php_fcgi;
pub mod postgresql;
pub mod profiles;
pub mod redis;
pub mod requests;
pub mod snapshots;
pub mod template;
pub mod tools;
pub mod web;
pub mod webhooks;

use lumina_core::types::ServiceStatus;
use lumina_process::{ProcessInfo, ProcessState};

/// UI status from the supervisor's view of a process. `failed_to_start` covers a
/// start that failed before any process was supervised.
/// What a service remembers about a failed start: nothing when there was
/// nothing to start (not installed), so its status stays "stopped".
pub(crate) fn start_failure<T>(result: &lumina_core::Result<T>) -> Option<String> {
    match result {
        Err(lumina_core::Error::NotInstalled { .. }) | Ok(_) => None,
        Err(err) => Some(err.to_string()),
    }
}

pub(crate) fn service_status(info: Option<&ProcessInfo>, failed_to_start: bool) -> ServiceStatus {
    match info.map(|i| i.state) {
        None if failed_to_start => ServiceStatus::Failed,
        None => ServiceStatus::Stopped,
        Some(ProcessState::Running) => ServiceStatus::Running,
        Some(ProcessState::Restarting) => ServiceStatus::Starting,
        Some(ProcessState::Failed) => ServiceStatus::Failed,
    }
}
