//! Shared domain types, error type and constants used by every Lumina crate.
//! This crate must stay dependency-light and free of OS-specific code.

pub mod constants;
pub mod error;
pub mod events;
pub mod fs;
pub mod paths;
pub mod types;

pub use error::{Error, Result};
