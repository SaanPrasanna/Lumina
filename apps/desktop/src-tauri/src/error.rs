//! Serializable command error returned to the UI (wraps lumina_core::Error).
//!
//! `code` is stable and translated on the frontend (`errors.<code>` in the locale
//! files); `message` is the English detail, shown as secondary text and logged.

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct CommandError {
    pub code: &'static str,
    pub message: String,
}

impl From<lumina_core::Error> for CommandError {
    fn from(err: lumina_core::Error) -> Self {
        use lumina_core::Error as E;
        let code = match &err {
            E::Io { .. } => "io",
            E::Config(_) => "config",
            E::PortInUse { .. } => "port_in_use",
            E::Service(..) => "service",
            E::ElevationRequired => "elevation_required",
            E::Other(_) => "other",
        };
        log::warn!("command failed [{code}]: {err}");
        Self {
            code,
            message: err.to_string(),
        }
    }
}

pub type CommandResult<T> = Result<T, CommandError>;
