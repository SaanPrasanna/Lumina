//! Which process is using a TCP port, for "port in use" error messages.

use std::process::Command;

use crate::command::hide_console;

/// PID of the process using local `port`: a listener if there is one, otherwise
/// any socket bound to it, e.g. another program's outgoing connection that got
/// the port as its source port (common when Windows' dynamic port range has been
/// widened to start at 1024). Best effort: `None` when the OS tool is missing or
/// access is denied.
pub fn port_user_pid(port: u16) -> Option<u32> {
    let mut cmd = listing_command(port);
    hide_console(&mut cmd);
    let output = cmd.output().ok()?;
    parse(&String::from_utf8_lossy(&output.stdout), port)
}

#[cfg(windows)]
fn listing_command(_port: u16) -> Command {
    let mut cmd = Command::new("netstat");
    cmd.args(["-ano", "-p", "TCP"]);
    cmd
}

#[cfg(not(windows))]
fn listing_command(port: u16) -> Command {
    // Listeners are listed first by `parse` preference; lsof prints PIDs only.
    let mut cmd = Command::new("lsof");
    cmd.args(["-nP", &format!("-iTCP:{port}"), "-t"]);
    cmd
}

/// `netstat -ano` rows: `TCP  127.0.0.1:10987  0.0.0.0:0  LISTENING  1234`.
/// A LISTENING row wins; otherwise any row whose *local* address uses the port.
#[cfg(windows)]
fn parse(output: &str, port: u16) -> Option<u32> {
    let suffix = format!(":{port}");
    let rows: Vec<(bool, u32)> = output
        .lines()
        .filter_map(|line| {
            let cols: Vec<&str> = line.split_whitespace().collect();
            match cols.as_slice() {
                [proto, local, _, state, pid]
                    if proto.eq_ignore_ascii_case("TCP") && local.ends_with(&suffix) =>
                {
                    Some((state.eq_ignore_ascii_case("LISTENING"), pid.parse().ok()?))
                }
                _ => None,
            }
        })
        .collect();
    rows.iter()
        .find(|(listening, _)| *listening)
        .or_else(|| rows.first())
        .map(|(_, pid)| *pid)
}

/// `lsof -t` prints one PID per line.
#[cfg(not(windows))]
fn parse(output: &str, _port: u16) -> Option<u32> {
    output.lines().find_map(|line| line.trim().parse().ok())
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn parses_netstat_rows() {
        let out = "\
  Proto  Local Address          Foreign Address        State           PID
  TCP    0.0.0.0:135            0.0.0.0:0              LISTENING       1100
  TCP    127.0.0.1:10987        127.0.0.1:50000        ESTABLISHED     42
  TCP    127.0.0.1:10987        0.0.0.0:0              LISTENING       4321
  TCP    [::]:109870            [::]:0                 LISTENING       9
";
        assert_eq!(parse(out, 10987), Some(4321), "listener preferred");
        assert_eq!(parse(out, 135), Some(1100));
        assert_eq!(parse(out, 80), None);
    }

    #[test]
    fn falls_back_to_an_outgoing_connection_using_the_port() {
        // Another program's outbound connection took 10987 as its source port.
        let out = "  TCP    192.168.8.100:10987    161.118.251.60:443     ESTABLISHED     10692
";
        assert_eq!(parse(out, 10987), Some(10692));
        // ...but a remote port of 10987 is not a local user.
        let remote = "  TCP    192.168.8.100:55000    1.2.3.4:10987     ESTABLISHED     7
";
        assert_eq!(parse(remote, 10987), None);
    }

    #[test]
    fn finds_a_real_listener() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        assert_eq!(port_user_pid(port), Some(std::process::id()));
    }
}
