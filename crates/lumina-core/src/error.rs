//! Crate-wide error type. Other crates wrap their errors into [`Error`].

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("configuration error: {0}")]
    Config(String),
    #[error("port {0} is already in use")]
    PortInUse(u16),
    #[error("service `{0}` failed: {1}")]
    Service(String, String),
    #[error("operation requires elevated privileges")]
    ElevationRequired,
    #[error("{0}")]
    Other(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
