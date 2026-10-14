//! Types and helpers shared by the web servers (nginx, Apache).

use std::path::{Path, PathBuf};

use lumina_config::schema::WebServer;
use lumina_core::types::ServiceStatus;
use lumina_core::{Error, Result};
use lumina_process::ports;
use serde::Serialize;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

/// What a web server config is rendered from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebContext {
    pub http_port: u16,
    /// Where PHP FastCGI listens (the actual port, which may be the fallback).
    pub php_port: u16,
    /// Served at http://localhost.
    pub www_root: PathBuf,
}

/// What is running right now.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebRuntime {
    pub server: WebServer,
    pub version: String,
    pub http_port: u16,
    pub php_port: u16,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebStatus {
    pub server: WebServer,
    pub status: ServiceStatus,
    pub runtime: Option<WebRuntime>,
    pub pid: Option<u32>,
    /// Live server processes (master + workers).
    pub processes: usize,
    pub error: Option<String>,
    /// The server's error log.
    pub log_file: PathBuf,
}

/// Result of a reload request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ReloadOutcome {
    /// Nothing changed on disk, so no signal was sent.
    Unchanged,
    /// The running server re-read its config; `signals` reload commands were
    /// sent (1 normally, 2 if the first was not confirmed).
    Reloaded { signals: u32 },
    /// The server was stopped and started (Apache on Windows has no graceful
    /// reload outside service mode).
    Restarted,
}

/// Fails with the owner's name if something already listens on `port`.
pub fn check_port(port: u16) -> Result<()> {
    if ports::is_free(port) {
        return Ok(());
    }
    Err(Error::PortInUse {
        port,
        owner: ports::owner(port),
    })
}

/// Puts Lumina's welcome page into `www_root` if it has no index file yet, so
/// http://localhost shows something on a fresh install. Never overwrites.
pub fn ensure_landing_page(www_root: &Path, bundled_page: &Path) -> Result<()> {
    std::fs::create_dir_all(www_root).map_err(|source| Error::Io {
        path: www_root.to_path_buf(),
        source,
    })?;
    let has_index = ["index.php", "index.html", "index.htm"]
        .iter()
        .any(|name| www_root.join(name).exists());
    if has_index {
        return Ok(());
    }
    let target = www_root.join("index.html");
    std::fs::copy(bundled_page, &target).map_err(|source| Error::Io {
        path: target,
        source,
    })?;
    Ok(())
}

/// The PIDs among `pids` whose executable name is `stem` (with or without
/// `.exe`). Process groups also contain helpers such as `conhost.exe` on
/// Windows, which must not be counted as server processes.
pub fn named(pids: &[u32], stem: &str) -> Vec<u32> {
    if pids.is_empty() {
        return Vec::new();
    }
    let sys_pids: Vec<Pid> = pids.iter().map(|&p| Pid::from_u32(p)).collect();
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::Some(&sys_pids),
        true,
        ProcessRefreshKind::nothing(),
    );
    let with_suffix = format!("{stem}{}", std::env::consts::EXE_SUFFIX);
    sys_pids
        .iter()
        .filter(|pid| {
            sys.process(**pid).is_some_and(|p| {
                let name = p.name().to_string_lossy();
                name.eq_ignore_ascii_case(stem) || name.eq_ignore_ascii_case(&with_suffix)
            })
        })
        .map(|pid| pid.as_u32())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn landing_page_is_copied_once_and_never_overwrites() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let page = tmp.path().join("bundled.html");
        std::fs::write(&page, "welcome").expect("write");
        let www = tmp.path().join("www");

        ensure_landing_page(&www, &page).expect("first");
        assert_eq!(
            std::fs::read_to_string(www.join("index.html")).expect("read"),
            "welcome"
        );

        std::fs::write(www.join("index.html"), "mine").expect("edit");
        ensure_landing_page(&www, &page).expect("second");
        assert_eq!(
            std::fs::read_to_string(www.join("index.html")).expect("read"),
            "mine"
        );
    }

    #[test]
    fn existing_php_index_means_no_landing_page() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let www = tmp.path().join("www");
        std::fs::create_dir_all(&www).expect("mkdir");
        std::fs::write(www.join("index.php"), "<?php").expect("write");
        ensure_landing_page(&www, &tmp.path().join("missing.html")).expect("noop");
        assert!(!www.join("index.html").exists());
    }

    #[test]
    fn busy_port_names_its_owner() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        match check_port(port) {
            Err(Error::PortInUse { port: p, owner }) => {
                assert_eq!(p, port);
                assert!(owner.is_some_and(|o| o.contains(&std::process::id().to_string())));
            }
            other => panic!("expected PortInUse, got {other:?}"),
        }
    }

    #[test]
    fn named_filters_by_executable() {
        let me = std::process::id();
        let exe = std::env::current_exe().expect("exe");
        let stem = exe
            .file_stem()
            .expect("stem")
            .to_string_lossy()
            .into_owned();
        assert_eq!(named(&[me], &stem), vec![me]);
        assert!(named(&[me], "definitely-not-this").is_empty());
    }
}
