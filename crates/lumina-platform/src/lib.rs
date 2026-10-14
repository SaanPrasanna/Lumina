//! The only crate allowed to contain `#[cfg(target_os = ...)]` logic.
//! Each area exposes one cross-platform API backed by per-OS modules.

pub mod command;
pub mod elevation;
pub mod hosts;
pub mod launcher;
pub mod net;
pub mod path_env;
pub mod process_group;
pub mod trust_store;
