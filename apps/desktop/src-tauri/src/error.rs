//! Serializable command error returned to the UI (wraps lumina_core::Error).
//!
//! `code` is stable and translated on the frontend (`errors.<code>` in the locale
//! files); `message` is the English detail, shown as secondary text and logged;
//! `details` carries structured data some errors need (e.g. which port is busy).

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct CommandError {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl From<lumina_core::Error> for CommandError {
    fn from(err: lumina_core::Error) -> Self {
        use lumina_core::Error as E;
        let (code, details) = match &err {
            E::Io { .. } => ("io", None),
            E::Config(_) => ("config", None),
            E::PortInUse { port, owner } => (
                "port_in_use",
                Some(serde_json::json!({ "port": port, "owner": owner })),
            ),
            E::Service(..) => ("service", None),
            E::ElevationRequired => ("elevation_required", None),
            E::Other(_) => ("other", None),
        };
        log::warn!("command failed [{code}]: {err}");
        Self {
            code,
            message: err.to_string(),
            details,
        }
    }
}

pub type CommandResult<T> = Result<T, CommandError>;
