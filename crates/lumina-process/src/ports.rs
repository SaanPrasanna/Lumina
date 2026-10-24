//! Port availability and fallback selection.

use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::time::Duration;

use lumina_core::{Error, Result};
use serde::Serialize;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

/// `true` if nothing is listening on `port` for local connections.
pub fn is_free(port: u16) -> bool {
    if TcpListener::bind((Ipv4Addr::LOCALHOST, port)).is_err() {
        return false;
    }
    // Windows lets us bind 127.0.0.1:p even while another socket holds 0.0.0.0:p,
    // so also check whether something answers.
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    TcpStream::connect_timeout(&addr, Duration::from_millis(150)).is_err()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PortChoice {
    pub port: u16,
    /// `true` when the preferred port was busy and the fallback was used.
    pub fell_back: bool,
}

/// Picks `preferred`, or `fallback` if `preferred` is busy. Fails with the owner of
/// the preferred port when both are taken.
pub fn pick(preferred: u16, fallback: u16) -> Result<PortChoice> {
    if is_free(preferred) {
        return Ok(PortChoice {
            port: preferred,
            fell_back: false,
        });
    }
    tracing::warn!(
        "port {preferred} is busy ({}); trying {fallback}",
        describe_owner(preferred)
    );
    if fallback != preferred && is_free(fallback) {
        return Ok(PortChoice {
            port: fallback,
            fell_back: true,
        });
    }
    Err(Error::PortInUse {
        port: preferred,
        owner: owner(preferred),
    })
}

/// "name (pid N)" of the process listening on `port`, if it can be determined.
pub fn owner(port: u16) -> Option<String> {
    let pid = lumina_platform::net::port_user_pid(port)?;
    let mut sys = System::new();
    let sys_pid = Pid::from_u32(pid);
    sys.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[sys_pid]),
        true,
        ProcessRefreshKind::nothing(),
    );
    let name = sys
        .process(sys_pid)
        .map(|p| p.name().to_string_lossy().into_owned());
    Some(match name {
        Some(name) => format!("{name} (pid {pid})"),
        None => format!("pid {pid}"),
    })
}

fn describe_owner(port: u16) -> String {
    owner(port).unwrap_or_else(|| "unknown process".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn busy_port() -> (TcpListener, u16) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        (listener, port)
    }

    // Below the ephemeral range, so a test binding port 0 at the same time
    // cannot be handed it between here and `pick`.
    fn free_port() -> u16 {
        static NEXT: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(20_000);
        loop {
            let port = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            assert!(port < 30_000, "no free port below the ephemeral range");
            if is_free(port) {
                return port;
            }
        }
    }

    #[test]
    fn prefers_the_preferred_port() {
        let port = free_port();
        assert_eq!(
            pick(port, free_port()).expect("pick"),
            PortChoice {
                port,
                fell_back: false
            }
        );
    }

    #[test]
    fn falls_back_when_preferred_is_busy() {
        let (_held, busy) = busy_port();
        let fallback = free_port();
        assert!(!is_free(busy));
        assert_eq!(
            pick(busy, fallback).expect("pick"),
            PortChoice {
                port: fallback,
                fell_back: true
            }
        );
    }

    #[test]
    fn errors_with_owner_when_both_are_busy() {
        let (_a, a) = busy_port();
        let (_b, b) = busy_port();
        match pick(a, b) {
            Err(Error::PortInUse { port, owner }) => {
                assert_eq!(port, a);
                // The listener is this test process.
                let owner = owner.expect("owner found");
                assert!(owner.contains(&std::process::id().to_string()), "{owner}");
            }
            other => panic!("expected PortInUse, got {other:?}"),
        }
    }
}
