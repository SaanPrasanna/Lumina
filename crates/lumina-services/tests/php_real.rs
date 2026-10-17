//! End-to-end checks against real php-cgi builds. Opt-in, because it needs PHP:
//!
//! ```powershell
//! $env:LUMINA_TEST_PHP_DIRS = "C:\Lumina\bin\php\8.3.35;C:\Lumina\bin\php\8.4.26"
//! cargo test -p lumina-services --test php_real -- --nocapture
//! ```
//!
//! The PHP folders are copied into a temporary data root, so the real one is
//! never touched. Without the variable every test returns early.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use lumina_config::schema::Php as PhpConfig;
use lumina_core::paths::Layout;
use lumina_core::types::ServiceStatus;
use lumina_process::Supervisor;
use lumina_process::ledger::Ledger;
use lumina_services::php_fcgi::PhpFcgi;
use lumina_services::template::Templates;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
use tempfile::TempDir;

struct Env {
    _tmp: TempDir,
    layout: Layout,
    php: PhpFcgi,
}

/// A data root containing copies of the PHP builds named in LUMINA_TEST_PHP_DIRS.
fn setup() -> Option<Env> {
    let dirs = std::env::var("LUMINA_TEST_PHP_DIRS").ok()?;
    let tmp = tempfile::tempdir().expect("tempdir");
    let layout = Layout::new(tmp.path().join("Lumina"));
    layout.ensure().expect("layout");
    for dir in std::env::split_paths(&dirs) {
        let name = dir.file_name().expect("dir name");
        copy_dir(&dir, &layout.bin().join("php").join(name));
    }
    let templates = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/templates");
    let php = PhpFcgi::new(
        Supervisor::new(Ledger::disabled()),
        layout.clone(),
        Templates::new(templates),
    );
    Some(Env {
        _tmp: tmp,
        layout,
        php,
    })
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("mkdir");
    for entry in std::fs::read_dir(from).expect("read_dir").flatten() {
        let target = to.join(entry.file_name());
        if entry.file_type().expect("type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).expect("copy");
        }
    }
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind")
        .local_addr()
        .expect("addr")
        .port()
}

fn config(version: &str, children: u32) -> PhpConfig {
    PhpConfig {
        version: version.into(),
        fcgi_port: free_port(),
        fcgi_fallback_port: free_port(),
        fcgi_children: children,
        fcgi_max_requests: 0,
    }
}

/// php-cgi processes whose executable is under `root`.
fn php_processes_under(root: &Path) -> Vec<u32> {
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_exe(UpdateKind::Always),
    );
    sys.processes()
        .iter()
        .filter(|(_, p)| p.exe().is_some_and(|e| e.starts_with(root)))
        .map(|(pid, _)| pid.as_u32())
        .collect()
}

fn write_script(layout: &Layout) -> PathBuf {
    let script = layout.www().join("info.php");
    std::fs::write(
        &script,
        "<?php echo PHP_VERSION, '|', basename(dirname(php_ini_loaded_file())), '|', \
         extension_loaded('mbstring') ? 'mbstring' : 'no-mbstring';",
    )
    .expect("script");
    script
}

/// Minimal FastCGI responder request; returns the response body.
fn fastcgi_get(port: u16, script: &Path) -> String {
    fn record(kind: u8, content: &[u8]) -> Vec<u8> {
        let len = u16::try_from(content.len()).expect("record fits");
        let mut r = vec![1, kind, 0, 1];
        r.extend_from_slice(&len.to_be_bytes());
        r.extend_from_slice(&[0, 0]);
        r.extend_from_slice(content);
        r
    }
    fn pair(name: &str, value: &str) -> Vec<u8> {
        let mut p = Vec::new();
        for len in [name.len(), value.len()] {
            if len < 128 {
                p.push(len as u8);
            } else {
                p.extend_from_slice(&(len as u32 | 0x8000_0000).to_be_bytes());
            }
        }
        p.extend_from_slice(name.as_bytes());
        p.extend_from_slice(value.as_bytes());
        p
    }

    let script = script.to_string_lossy().replace('\\', "/");
    let params: Vec<u8> = [
        ("SCRIPT_FILENAME", script.as_str()),
        ("SCRIPT_NAME", "/info.php"),
        ("REQUEST_METHOD", "GET"),
        ("QUERY_STRING", ""),
        ("SERVER_PROTOCOL", "HTTP/1.1"),
        ("GATEWAY_INTERFACE", "CGI/1.1"),
        ("REDIRECT_STATUS", "200"),
    ]
    .iter()
    .flat_map(|(k, v)| pair(k, v))
    .collect();

    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("timeout");
    let mut request = record(1, &[0, 1, 0, 0, 0, 0, 0, 0]); // BEGIN_REQUEST, responder
    request.extend(record(4, &params));
    request.extend(record(4, &[]));
    request.extend(record(5, &[])); // empty STDIN
    stream.write_all(&request).expect("write");

    let mut stdout = Vec::new();
    loop {
        let mut header = [0u8; 8];
        stream.read_exact(&mut header).expect("record header");
        let len = u16::from_be_bytes([header[4], header[5]]) as usize + header[6] as usize;
        let mut body = vec![0u8; len];
        stream.read_exact(&mut body).expect("record body");
        match header[1] {
            6 => stdout.extend_from_slice(&body[..len - header[6] as usize]),
            3 => break, // END_REQUEST
            _ => {}
        }
    }
    let text = String::from_utf8_lossy(&stdout).into_owned();
    // Drop the CGI headers.
    text.split_once("\r\n\r\n")
        .map_or(text.clone(), |(_, b)| b.to_owned())
}

#[tokio::test]
async fn serves_requests_with_enforced_children_and_our_php_ini() {
    let Some(env) = setup() else { return };
    let script = write_script(&env.layout);

    let runtime = env.php.start(&config("8.3", 2), &[]).await.expect("start");

    assert_eq!(runtime.children, 4, "PHP_FCGI_CHILDREN=2 raised to 4");
    let body = fastcgi_get(runtime.port, &script);
    println!("response: {body}");
    assert!(body.starts_with("8.3."), "{body}");
    assert!(
        body.contains(&format!("|{}|", runtime.version)),
        "our php.ini: {body}"
    );
    assert!(body.ends_with("|mbstring"), "extensions enabled: {body}");

    // Master + 4 workers.
    let status = env.php.status().await;
    println!("status: {status:?}");
    assert_eq!(status.status, ServiceStatus::Running);
    assert_eq!(status.processes, 5);

    env.php.stop().await.expect("stop");
    assert!(php_processes_under(&env.layout.bin()).is_empty());
}

#[tokio::test]
async fn version_switch_leaves_zero_old_processes() {
    let Some(env) = setup() else { return };
    let script = write_script(&env.layout);
    let cfg83 = config("8.3", 4);
    let old_root = env.layout.bin().join("php").join(
        env.php
            .versions()
            .await
            .iter()
            .find(|i| i.version.starts_with("8.3."))
            .expect("8.3 installed")
            .dir
            .file_name()
            .expect("name"),
    );

    let first = env.php.start(&cfg83, &[]).await.expect("start 8.3");
    assert!(fastcgi_get(first.port, &script).starts_with("8.3."));
    assert_eq!(php_processes_under(&old_root).len(), 5);

    let cfg84 = PhpConfig {
        version: "8.4".into(),
        ..cfg83
    };
    let second = env.php.restart(&cfg84, &[]).await.expect("switch to 8.4");

    assert!(
        php_processes_under(&old_root).is_empty(),
        "8.3 ghosts after switch"
    );
    assert!(fastcgi_get(second.port, &script).starts_with("8.4."));
    env.php.stop().await.expect("stop");
    assert!(php_processes_under(&env.layout.bin()).is_empty());
}

#[tokio::test]
async fn falls_back_when_the_main_port_is_taken() {
    let Some(env) = setup() else { return };
    let blocker = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let mut cfg = config("", 4);
    cfg.fcgi_port = blocker.local_addr().expect("addr").port();

    let runtime = env.php.start(&cfg, &[]).await.expect("start");

    assert!(runtime.fell_back);
    assert_eq!(runtime.port, cfg.fcgi_fallback_port);
    env.php.stop().await.expect("stop");
}

#[tokio::test]
async fn missing_version_is_a_clear_error() {
    let Some(env) = setup() else { return };
    let err = env
        .php
        .start(&config("5.6", 4), &[])
        .await
        .expect_err("not installed");
    assert!(
        err.to_string().contains("PHP 5.6 is not installed"),
        "{err}"
    );
    assert_eq!(env.php.status().await.status, ServiceStatus::Failed);
}
