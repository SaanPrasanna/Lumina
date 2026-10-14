//! Readiness probes.

use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use tokio::net::TcpStream;
use tokio::time::{Instant, sleep, timeout};

/// Waits until something accepts TCP connections on `127.0.0.1:port`.
/// Returns `false` if `limit` elapses first.
pub async fn wait_tcp(port: u16, limit: Duration) -> bool {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let deadline = Instant::now() + limit;
    while Instant::now() < deadline {
        if let Ok(Ok(_)) = timeout(Duration::from_millis(250), TcpStream::connect(addr)).await {
            return true;
        }
        sleep(Duration::from_millis(100)).await;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn detects_listener_and_times_out_without_one() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        assert!(wait_tcp(port, Duration::from_secs(2)).await);

        drop(listener);
        assert!(!wait_tcp(port, Duration::from_millis(300)).await);
    }
}
