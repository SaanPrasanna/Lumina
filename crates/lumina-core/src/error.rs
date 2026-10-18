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
    #[error("port {port} is already in use{}", owner.as_deref().map(|o| format!(" by {o}")).unwrap_or_default())]
    PortInUse { port: u16, owner: Option<String> },
    #[error("service `{0}` failed: {1}")]
    Service(String, String),
    /// Nothing to run: `name` (at `version`, if one was asked for) is not
    /// installed. `package` is its Quick Add id.
    #[error("{name}{} is not installed", if version.is_empty() { String::new() } else { format!(" {version}") })]
    NotInstalled {
        service: String,
        package: String,
        name: String,
        version: String,
    },
    #[error("operation requires elevated privileges")]
    ElevationRequired,
    #[error("{0}")]
    Other(String),
}

impl Error {
    /// [`Error::NotInstalled`] for the Quick Add package `package`, any version.
    pub fn not_installed(package: &str, name: &str) -> Self {
        Self::NotInstalled {
            service: package.into(),
            package: package.into(),
            name: name.into(),
            version: String::new(),
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
