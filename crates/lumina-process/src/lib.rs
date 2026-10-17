//! Generic process supervision used by every service: spawn into a kill-on-close
//! group, log to a file, restart on crash, and stop with verification that nothing
//! is left running.

pub mod handle;
pub mod health;
pub mod ledger;
pub mod logs;
pub mod ports;
pub mod supervisor;
pub mod wake;

pub use handle::{ProcessEvent, ProcessInfo, ProcessSpec, ProcessState, RestartPolicy};
pub use supervisor::Supervisor;
