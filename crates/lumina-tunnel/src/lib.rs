//! Tunnel providers. Ngrok first; the trait allows adding others (e.g. Cloudflare Tunnel) later.
//!
//! A tunnel forwards a public HTTPS address to a site's plain-HTTP virtual
//! host on 127.0.0.1. The provider rewrites the `Host` header to the site's
//! local domain (so the web server picks the right vhost) and adds
//! [`ngrok::HEADER`], which the web server templates use to give PHP the
//! public host and `HTTPS=on`: links, assets and redirects then use the
//! public address instead of `<site>.test`.

pub mod ngrok;
pub mod provider;
pub mod qr;
pub mod token;

use std::path::PathBuf;

pub use provider::{Tunnel, TunnelProvider, TunnelSpec};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no ngrok authtoken is saved")]
    NoToken,
    #[error("{0}")]
    InvalidToken(String),
    /// The provider refused the credentials (wrong, revoked or expired token).
    #[error("ngrok refused the authtoken: {0}")]
    Auth(String),
    /// The agent did not start, or stopped.
    #[error("the ngrok agent stopped: {0}")]
    Agent(String),
    /// The agent runs but cannot reach ngrok's servers.
    #[error("could not connect to ngrok: {0}")]
    Connect(String),
    /// The agent runs but refused the tunnel (e.g. a domain the account does not own).
    #[error("ngrok could not open the tunnel: {0}")]
    Api(String),
    #[error("the system keychain: {0}")]
    Keychain(String),
    #[error("{0}")]
    Invalid(String),
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Core(#[from] lumina_core::Error),
}

impl Error {
    /// Stable code for the UI (`errors.<code>` in the locale files).
    pub fn code(&self) -> &'static str {
        match self {
            Self::NoToken => "tunnel_no_token",
            Self::InvalidToken(_) => "tunnel_invalid_token",
            Self::Auth(_) => "tunnel_auth",
            Self::Agent(_) | Self::Connect(_) | Self::Api(_) | Self::Invalid(_) => "tunnel",
            Self::Keychain(_) => "keychain",
            Self::Io { .. } => "io",
            Self::Core(_) => "other",
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
