//! The TunnelProvider trait.

use std::future::Future;

use serde::Serialize;

use crate::Result;

/// What to share.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunnelSpec {
    /// Unique name, the site's folder name (`blog`).
    pub name: String,
    /// The site's local domain (`blog.test`), sent as the `Host` header.
    pub host: String,
    /// The web server's plain-HTTP port on 127.0.0.1.
    pub port: u16,
    /// A reserved public domain (`my-app.ngrok.app`); `None` = a random one.
    pub domain: Option<String>,
}

/// An open tunnel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tunnel {
    pub name: String,
    /// `https://abc123.ngrok-free.app`.
    pub public_url: String,
    pub host: String,
    pub port: u16,
    /// Unix seconds.
    pub started_at: u64,
}

/// A way to make local sites reachable from the internet.
pub trait TunnelProvider: Send + Sync {
    /// Credentials and the program to run, read fresh for every `open`.
    type Settings: Send + Sync;

    /// Opens a tunnel for `spec` (or returns the one already open under its name).
    fn open(
        &self,
        settings: &Self::Settings,
        spec: &TunnelSpec,
    ) -> impl Future<Output = Result<Tunnel>> + Send;

    /// Closes one tunnel; closing the last one stops the agent.
    fn close(&self, name: &str) -> impl Future<Output = Result<()>> + Send;

    /// Open tunnels (empty if the agent stopped meanwhile).
    fn tunnels(&self) -> impl Future<Output = Vec<Tunnel>> + Send;

    /// Closes everything and stops the agent (app exit).
    fn shutdown(&self) -> impl Future<Output = Result<()>> + Send;
}
