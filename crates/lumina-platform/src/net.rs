//! Which process is listening on a TCP port, for "port in use" error messages.

use std::process::Command;

use crate::command::hide_console;

/// PID of a process listening on `port` (any local address), if one can be found.
/// Best effort: returns `None` when the OS tool is missing or access is denied.
pub fn listening_pid(port: u16) -> Option<u32> {
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
    let mut cmd = Command::new("lsof");
    cmd.args(["-nP", &format!("-iTCP:{port}"), "-sTCP:LISTEN", "-t"]);
    cmd
}

/// `netstat -ano` rows: `TCP  127.0.0.1:10987  0.0.0.0:0  LISTENING  1234`.
#[cfg(windows)]
fn parse(output: &str, port: u16) -> Option<u32> {
    let suffix = format!(":{port}");
    output.lines().find_map(|line| {
        let cols: Vec<&str> = line.split_whitespace().collect();
        match cols.as_slice() {
            [proto, local, _, state, pid]
                if proto.eq_ignore_ascii_case("TCP")
                    && state.eq_ignore_ascii_case("LISTENING")
                    && local.ends_with(&suffix) =>
            {
                pid.parse().ok()
            }
            _ => None,
        }
    })
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
    fn parses_netstat_listening_rows_only() {
        let out = "\
  Proto  Local Address          Foreign Address        State           PID
  TCP    0.0.0.0:135            0.0.0.0:0              LISTENING       1100
  TCP    127.0.0.1:10987        127.0.0.1:50000        ESTABLISHED     42
  TCP    127.0.0.1:10987        0.0.0.0:0              LISTENING       4321
  TCP    [::]:109870            [::]:0                 LISTENING       9
";
        assert_eq!(parse(out, 10987), Some(4321));
        assert_eq!(parse(out, 135), Some(1100));
        assert_eq!(parse(out, 80), None);
    }

    #[test]
    fn finds_a_real_listener() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        assert_eq!(listening_pid(port), Some(std::process::id()));
    }
}
