//! Concrete service drivers built on `lumina-process`, plus the orchestrator that
//! starts/stops them as a stack and switches profiles.

pub mod apache;
pub mod database;
pub mod install;
pub mod nginx;
pub mod orchestrator;
pub mod php_fcgi;
pub mod postgresql;
pub mod profiles;
pub mod redis;
pub mod template;
pub mod tools;
pub mod web;

use lumina_core::types::ServiceStatus;
use lumina_process::{ProcessInfo, ProcessState};

/// UI status from the supervisor's view of a process. `failed_to_start` covers a
/// start that failed before any process was supervised.
pub(crate) fn service_status(info: Option<&ProcessInfo>, failed_to_start: bool) -> ServiceStatus {
    match info.map(|i| i.state) {
        None if failed_to_start => ServiceStatus::Failed,
        None => ServiceStatus::Stopped,
        Some(ProcessState::Running) => ServiceStatus::Running,
        Some(ProcessState::Restarting) => ServiceStatus::Starting,
        Some(ProcessState::Failed) => ServiceStatus::Failed,
    }
}
