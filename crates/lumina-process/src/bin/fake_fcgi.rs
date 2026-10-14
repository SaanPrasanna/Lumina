//! Test fixture imitating `php-cgi -b` with `PHP_FCGI_CHILDREN`: the parent spawns
//! N worker copies of itself and optionally listens on a port, then runs forever.
//!
//! Environment:
//! - `FAKE_CHILDREN`      number of workers (default 0)
//! - `FAKE_PORT`          listen on 127.0.0.1:<port> (parent only)
//! - `FAKE_EXIT_AFTER_MS` parent exits with code 3 after this delay (crash simulation)
//!
//! Workers are started with `--worker` and only sleep. They are deliberately not
//! cleaned up by the parent, so a test fails if the supervisor leaves them behind.

use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

fn env_num(name: &str) -> Option<u64> {
    std::env::var(name).ok()?.parse().ok()
}

fn main() {
    if std::env::args().any(|a| a == "--worker") {
        loop {
            sleep(Duration::from_secs(3600));
        }
    }

    let _listener = env_num("FAKE_PORT").map(|port| {
        std::net::TcpListener::bind(("127.0.0.1", port as u16)).expect("bind FAKE_PORT")
    });

    let exe = std::env::current_exe().expect("current exe");
    let mut workers = Vec::new();
    for _ in 0..env_num("FAKE_CHILDREN").unwrap_or(0) {
        let mut cmd = Command::new(&exe);
        cmd.arg("--worker").stdin(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        workers.push(cmd.spawn().expect("spawn worker"));
    }
    println!(
        "fake-fcgi: parent {} with {} workers",
        std::process::id(),
        workers.len()
    );

    if let Some(ms) = env_num("FAKE_EXIT_AFTER_MS") {
        sleep(Duration::from_millis(ms));
        std::process::exit(3);
    }
    loop {
        sleep(Duration::from_secs(3600));
    }
}
