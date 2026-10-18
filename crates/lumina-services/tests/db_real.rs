//! End-to-end checks with real MySQL, MariaDB, Redis, PostgreSQL and PHP builds. Opt-in:
//!
//! ```powershell
//! $env:LUMINA_TEST_PHP_DIRS     = "C:\Lumina\bin\php\8.4.26"
//! $env:LUMINA_TEST_MYSQL_DIR    = "C:\Lumina\bin\mysql\8.4.11"
//! $env:LUMINA_TEST_MARIADB_DIR  = "C:\Lumina\bin\mariadb\12.3.3"
//! $env:LUMINA_TEST_REDIS_DIR    = "C:\Lumina\bin\redis\8.10.2"
//! $env:LUMINA_TEST_POSTGRES_DIR = "C:\Lumina\bin\postgresql\18.6"
//! $env:LUMINA_TEST_MAILPIT_DIR  = "C:\Lumina\bin\mailpit\1.31.6"
//! cargo test -p lumina-services --test db_real -- --nocapture --test-threads=1
//! ```
//!
//! The installs are linked (not copied: MySQL is over 1 GB) into a temporary
//! data root; data directories live there, so the real ones are untouched.
//! Tests whose variables are missing return early.

use std::path::{Path, PathBuf};
use std::process::Command;

use lumina_config::AppConfig;
use lumina_config::schema::DatabaseEngine;
use lumina_core::paths::Layout;
use lumina_core::types::ServiceKind;
use lumina_process::Supervisor;
use lumina_process::ledger::Ledger;
use lumina_services::orchestrator::Orchestrator;
use lumina_services::template::Templates;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
use tempfile::TempDir;

struct Env {
    _tmp: TempDir,
    layout: Layout,
    stack: Orchestrator,
    cfg: AppConfig,
}

/// Links `source` at `target` without copying (a junction on Windows).
fn link_dir(source: &Path, target: &Path) {
    std::fs::create_dir_all(target.parent().expect("parent")).expect("mkdir");
    #[cfg(windows)]
    {
        let status = Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(target)
            .arg(source)
            .output()
            .expect("mklink");
        assert!(status.status.success(), "mklink /J failed: {status:?}");
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(source, target).expect("symlink");
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind")
        .local_addr()
        .expect("addr")
        .port()
}

/// A data root with the tools named by `vars` (env var → bin subfolder) linked in.
fn setup(vars: &[(&str, &str)]) -> Option<Env> {
    let mut tools = Vec::new();
    for (var, tool) in vars {
        tools.push((PathBuf::from(std::env::var(var).ok()?), *tool));
    }
    let tmp = tempfile::tempdir().expect("tempdir");
    let layout = Layout::new(tmp.path().join("Lumina"));
    layout.ensure().expect("layout");
    for (dir, tool) in tools {
        link_dir(
            &dir,
            &layout.bin().join(tool).join(dir.file_name().expect("name")),
        );
    }
    let mut cfg = AppConfig::default();
    cfg.database.port = free_port();
    cfg.redis.port = free_port();
    cfg.postgresql.port = free_port();
    let templates = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/templates");
    let stack = Orchestrator::new(
        Supervisor::new(Ledger::disabled()),
        layout.clone(),
        Templates::new(templates),
    );
    Some(Env {
        _tmp: tmp,
        layout,
        stack,
        cfg,
    })
}

/// Live processes whose executable is under `root` (follows the junction).
fn processes_under(root: &Path) -> Vec<u32> {
    let root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_exe(UpdateKind::Always),
    );
    sys.processes()
        .iter()
        .filter(|(_, p)| {
            p.exe().is_some_and(|e| {
                std::fs::canonicalize(e)
                    .unwrap_or_else(|_| e.to_path_buf())
                    .starts_with(&root)
            })
        })
        .map(|(pid, _)| pid.as_u32())
        .collect()
}

/// Connects with PHP's mysqli as root@127.0.0.1 (empty password), the way a
/// Laravel or WordPress app would, and returns the server version.
fn php_connects(port: u16) -> String {
    let php_dir = std::env::var("LUMINA_TEST_PHP_DIRS")
        .ok()
        .and_then(|v| std::env::split_paths(&v).next())
        .expect("LUMINA_TEST_PHP_DIRS");
    let php = php_dir.join(format!("php{}", std::env::consts::EXE_SUFFIX));
    let code = format!(
        "$m = new mysqli('127.0.0.1', 'root', '', '', {port}); \
         $m->query('CREATE DATABASE IF NOT EXISTS lumina_test'); \
         echo $m->server_info;"
    );
    let out = Command::new(php)
        .args(["-n", "-d"])
        .arg(format!("extension_dir={}", php_dir.join("ext").display()))
        .args(["-d", "extension=mysqli", "-r", &code])
        .output()
        .expect("run php");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        out.status.success(),
        "PHP could not connect: {stdout}{}",
        String::from_utf8_lossy(&out.stderr)
    );
    stdout
}

/// Error-log text written since `from` bytes.
fn log_since(path: &Path, from: usize) -> String {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    text.get(from..).unwrap_or_default().to_owned()
}

async fn database_lifecycle(engine: DatabaseEngine, var: &str, tool: &str) {
    let Some(mut env) = setup(&[(var, tool)]) else {
        return;
    };
    if std::env::var("LUMINA_TEST_PHP_DIRS").is_err() {
        return;
    }
    env.cfg.database.engine = engine;
    let kind = Orchestrator::configured_db(&env.cfg);
    let db = env.stack.database(engine);

    // First start creates the data directory.
    env.stack.start(kind, &env.cfg).await.expect("first start");
    let status = db.status().await;
    let rt = status.runtime.expect("running");
    assert!(
        rt.data_dir.join("mysql").is_dir(),
        "initialised: {}",
        rt.data_dir.display()
    );
    let version = php_connects(env.cfg.database.port);
    println!("{engine:?}: PHP connected to {version}");
    assert!(
        version.starts_with(&rt.version),
        "{version} vs {}",
        rt.version
    );

    env.stack.stop(kind).await.expect("graceful stop");
    assert!(
        processes_under(&env.layout.bin().join(tool)).is_empty(),
        "leftovers"
    );

    // A clean shutdown means no crash recovery on the next start.
    let log = db.log_file();
    let before = std::fs::read_to_string(&log).map(|t| t.len()).unwrap_or(0);
    env.stack.start(kind, &env.cfg).await.expect("second start");
    php_connects(env.cfg.database.port);
    let restart_log = log_since(&log, before);
    for sign in ["not shutdown normally", "crash recovery", "Crash recovery"] {
        assert!(
            !restart_log.contains(sign),
            "found {sign:?} after restart:\n{restart_log}"
        );
    }
    env.stack.stop(kind).await.expect("stop");
    assert!(
        processes_under(&env.layout.bin().join(tool)).is_empty(),
        "leftovers"
    );
}

#[tokio::test]
async fn mysql_initialises_serves_php_and_stops_cleanly() {
    database_lifecycle(DatabaseEngine::Mysql, "LUMINA_TEST_MYSQL_DIR", "mysql").await;
}

#[tokio::test]
async fn mariadb_initialises_serves_php_and_stops_cleanly() {
    database_lifecycle(
        DatabaseEngine::Mariadb,
        "LUMINA_TEST_MARIADB_DIR",
        "mariadb",
    )
    .await;
}

#[tokio::test]
async fn starting_one_engine_stops_the_other() {
    let Some(mut env) = setup(&[
        ("LUMINA_TEST_MYSQL_DIR", "mysql"),
        ("LUMINA_TEST_MARIADB_DIR", "mariadb"),
    ]) else {
        return;
    };
    env.stack
        .start(ServiceKind::Mysql, &env.cfg)
        .await
        .expect("mysql");
    env.cfg.database.engine = DatabaseEngine::Mariadb;
    env.stack
        .start(ServiceKind::Mariadb, &env.cfg)
        .await
        .expect("mariadb");

    assert!(
        processes_under(&env.layout.bin().join("mysql")).is_empty(),
        "MySQL still running"
    );
    let status = env.stack.status(&env.cfg).await;
    assert_eq!(status.database.engine, DatabaseEngine::Mariadb);
    assert!(status.database.runtime.is_some());
    // Each engine keeps its own data directory.
    assert!(env.layout.data().read_dir().expect("data").count() >= 2);
    env.stack.stop_all().await;
    assert!(processes_under(&env.layout.bin()).is_empty());
}

/// Runs one SQL statement with the server's own client; returns its output.
fn sql(server: &lumina_services::snapshots::Server, db: &str, statement: &str) -> String {
    let exe = std::env::consts::EXE_SUFFIX;
    let port = server.port.to_string();
    let out = if server.engine == "postgresql" {
        Command::new(server.bin.join(format!("psql{exe}")))
            .args([
                "-h",
                "127.0.0.1",
                "-p",
                &port,
                "-U",
                "postgres",
                "-At",
                "-d",
                db,
                "-c",
                statement,
            ])
            .output()
    } else {
        Command::new(server.bin.join(format!("mysql{exe}")))
            .args([
                "--host=127.0.0.1",
                &format!("--port={port}"),
                "--user=root",
                "-N",
                "-e",
                statement,
                db,
            ])
            .output()
    }
    .expect("client");
    assert!(
        out.status.success(),
        "{statement}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// Snapshot, change, restore: the database is back as it was.
async fn snapshot_round_trip(var: &str, tool: &str, engine: &str) {
    let Some(mut env) = setup(&[(var, tool)]) else {
        return;
    };
    let kind = match engine {
        "postgresql" => ServiceKind::Postgresql,
        _ => {
            env.cfg.database.engine = DatabaseEngine::Mariadb;
            ServiceKind::Mariadb
        }
    };
    env.stack.start(kind, &env.cfg).await.expect("start");
    let server = env.stack.snapshot_server(engine).await.expect("server");
    let admin_db = if engine == "postgresql" {
        "postgres"
    } else {
        ""
    };
    sql(&server, admin_db, "CREATE DATABASE lumina_snap");
    sql(
        &server,
        "lumina_snap",
        "CREATE TABLE notes (body VARCHAR(20))",
    );
    sql(&server, "lumina_snap", "INSERT INTO notes VALUES ('kept')");

    let dbs = lumina_services::snapshots::databases(&server)
        .await
        .expect("databases");
    assert!(dbs.contains(&"lumina_snap".to_owned()), "{dbs:?}");
    assert!(
        !dbs.iter().any(|d| d == "mysql" || d == "postgres"),
        "system databases hidden: {dbs:?}"
    );

    let snap = lumina_services::snapshots::create(&env.layout, &server, "lumina_snap")
        .await
        .expect("snapshot");
    assert!(snap.size > 0);
    sql(&server, "lumina_snap", "INSERT INTO notes VALUES ('lost')");
    sql(&server, "lumina_snap", "CREATE TABLE extra (x INT)");

    lumina_services::snapshots::restore(&env.layout, &server, &snap.id)
        .await
        .expect("restore");
    assert_eq!(
        sql(&server, "lumina_snap", "SELECT body FROM notes"),
        "kept"
    );
    let tables = if engine == "postgresql" {
        sql(
            &server,
            "lumina_snap",
            "SELECT count(*) FROM pg_tables WHERE tablename = 'extra'",
        )
    } else {
        sql(
            &server,
            "lumina_snap",
            "SELECT count(*) FROM information_schema.tables WHERE table_name = 'extra'",
        )
    };
    assert_eq!(tables, "0", "the restore replaced the whole database");

    assert_eq!(lumina_services::snapshots::list(&env.layout).len(), 1);
    lumina_services::snapshots::delete(&env.layout, &snap.id).expect("delete");
    env.stack.stop(kind).await.expect("stop");
}

#[tokio::test]
async fn mariadb_snapshot_round_trip() {
    snapshot_round_trip("LUMINA_TEST_MARIADB_DIR", "mariadb", "mariadb").await;
}

#[tokio::test]
async fn postgresql_snapshot_round_trip() {
    snapshot_round_trip("LUMINA_TEST_POSTGRES_DIR", "postgresql", "postgresql").await;
}

/// MongoDB, Meilisearch and SeaweedFS from `LUMINA_TEST_EXTRA_BIN` (a `bin/`
/// folder with `<id>/<version>/` installs, e.g. the one the catalog_real test
/// of lumina-packages keeps): each starts on its port and does its job.
#[tokio::test]
async fn extra_services_start_work_and_stop() {
    let Some(bin) = std::env::var_os("LUMINA_TEST_EXTRA_BIN").map(PathBuf::from) else {
        return;
    };
    let Some(mut env) = setup(&[]) else { return };
    let mut started = Vec::new();
    for def in lumina_services::extra::DEFINITIONS {
        let Some(install) = std::fs::read_dir(bin.join(def.id))
            .ok()
            .and_then(|mut d| d.next())
            .and_then(Result::ok)
        else {
            continue;
        };
        link_dir(
            &install.path(),
            &env.layout.bin().join(def.id).join(install.file_name()),
        );
        env.cfg.services.insert(
            def.id.into(),
            lumina_config::schema::ExtraService {
                enabled: true,
                port: free_port(),
                ..Default::default()
            },
        );
        started.push(def.id);
    }
    assert!(!started.is_empty(), "no installs in {}", bin.display());

    // Start All starts every enabled one.
    env.stack.start_all(&env.cfg).await.ok();
    let port = |id: &str| env.cfg.services[id].port;
    let http = reqwest::Client::new();
    for status in env.stack.extras_status(&env.cfg).await {
        if !started.contains(&status.id.as_str()) {
            continue;
        }
        assert!(
            status.runtime.is_some(),
            "{} did not start: {:?}",
            status.id,
            status.error
        );
    }
    if started.contains(&"meilisearch") {
        let body = http
            .get(format!("http://127.0.0.1:{}/health", port("meilisearch")))
            .send()
            .await
            .expect("health")
            .text()
            .await
            .expect("body");
        assert!(body.contains("available"), "{body}");
    }
    if started.contains(&"seaweedfs") {
        // S3 without credentials: create a bucket, store an object, read it back.
        let base = format!("http://127.0.0.1:{}", port("seaweedfs"));
        let put = http
            .put(format!("{base}/lumina-test"))
            .send()
            .await
            .expect("bucket");
        assert!(put.status().is_success(), "create bucket: {}", put.status());
        let put = http
            .put(format!("{base}/lumina-test/hello.txt"))
            .body("hello from lumina")
            .send()
            .await
            .expect("put");
        assert!(put.status().is_success(), "put object: {}", put.status());
        let got = http
            .get(format!("{base}/lumina-test/hello.txt"))
            .send()
            .await
            .expect("get")
            .text()
            .await
            .expect("body");
        assert_eq!(got, "hello from lumina");
    }
    if started.contains(&"mongodb") {
        assert!(std::net::TcpStream::connect(("127.0.0.1", port("mongodb"))).is_ok());
        let log =
            std::fs::read_to_string(env.layout.logs().join("mongodb.log")).unwrap_or_default();
        assert!(log.contains("Waiting for connections"), "{log}");
    }

    let failures = env.stack.stop_all().await;
    assert!(failures.is_empty(), "{failures:?}");
    for id in started {
        assert!(
            processes_under(&env.layout.bin().join(id)).is_empty(),
            "{id} left processes behind"
        );
    }
}

#[tokio::test]
async fn mailpit_catches_smtp_and_php_mail() {
    let Some(mut env) = setup(&[("LUMINA_TEST_MAILPIT_DIR", "mailpit")]) else {
        return;
    };
    env.cfg.mailpit.smtp_port = free_port();
    env.cfg.mailpit.ui_port = free_port();
    env.stack
        .start(ServiceKind::Mailpit, &env.cfg)
        .await
        .expect("start");
    let smtp = env.cfg.mailpit.smtp_port;

    // A plain SMTP client with a login, as Laravel or WordPress SMTP plugins send.
    {
        use std::io::{BufRead, BufReader, Write};
        let stream = std::net::TcpStream::connect(("127.0.0.1", smtp)).expect("connect");
        let mut reader = BufReader::new(stream.try_clone().expect("clone"));
        let mut writer = stream;
        let mut expect = |code: &str| loop {
            let mut line = String::new();
            reader.read_line(&mut line).expect("read");
            assert!(line.starts_with(code), "expected {code}, got {line:?}");
            if line.as_bytes().get(3) != Some(&b'-') {
                break;
            }
        };
        expect("220");
        for (command, code) in [
            ("EHLO lumina.test", "250"),
            ("AUTH PLAIN AHVzZXIAc2VjcmV0", "235"), // user / secret: any login works
            ("MAIL FROM:<app@lumina.test>", "250"),
            ("RCPT TO:<someone@example.com>", "250"),
            ("DATA", "354"),
            ("Subject: From SMTP\r\n\r\nHello.\r\n.", "250"),
            ("QUIT", "221"),
        ] {
            write!(writer, "{command}\r\n").expect("write");
            expect(code);
        }
    }

    // PHP's mail(), with the settings Lumina gives php-cgi and the `php` command.
    if let Some(php_dir) =
        std::env::var_os("LUMINA_TEST_PHP_DIRS").and_then(|v| std::env::split_paths(&v).next())
    {
        let php = php_dir.join(format!("php{}", std::env::consts::EXE_SUFFIX));
        let ini = lumina_services::php_fcgi::ini_args(&env.stack.php_mail_ini(&env.cfg).await);
        let out = Command::new(php)
            .arg("-n")
            .args(&ini)
            .args([
                "-r",
                "var_export(mail('someone@example.com', 'From PHP', 'Hello.', 'From: app@lumina.test'));",
            ])
            .output()
            .expect("run php");
        assert_eq!(
            String::from_utf8_lossy(&out.stdout),
            "true",
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    let api = format!(
        "http://127.0.0.1:{}/api/v1/messages",
        env.cfg.mailpit.ui_port
    );
    let inbox = reqwest::get(api)
        .await
        .expect("api")
        .text()
        .await
        .expect("body");
    assert!(inbox.contains("From SMTP"), "{inbox}");
    if std::env::var_os("LUMINA_TEST_PHP_DIRS").is_some() {
        assert!(inbox.contains("From PHP"), "{inbox}");
    }

    env.stack.stop(ServiceKind::Mailpit).await.expect("stop");
    assert!(
        processes_under(&env.layout.bin().join("mailpit")).is_empty(),
        "leftovers"
    );
    assert!(env.layout.data().join("mailpit/mailpit.db").is_file());
}

#[tokio::test]
async fn heal_restarts_a_service_that_stopped_answering() {
    let Some(env) = setup(&[("LUMINA_TEST_REDIS_DIR", "redis")]) else {
        return;
    };
    env.stack
        .start(ServiceKind::Redis, &env.cfg)
        .await
        .expect("start");
    let limit = std::time::Duration::from_secs(2);
    assert!(
        env.stack.heal(&env.cfg, limit).await.is_empty(),
        "a healthy service is left alone"
    );

    // Redis keeps running but closes its TCP listener.
    let dir = PathBuf::from(std::env::var("LUMINA_TEST_REDIS_DIR").expect("var"));
    let cli = dir.join(format!("redis-cli{}", std::env::consts::EXE_SUFFIX));
    let port = env.cfg.redis.port.to_string();
    let out = Command::new(&cli)
        .args(["-p", &port, "CONFIG", "SET", "port", "0"])
        .output()
        .expect("cli");
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "OK");
    let before = env.stack.redis().status().await.pid;

    let healed = env.stack.heal(&env.cfg, limit).await;
    assert_eq!(healed.len(), 1, "{healed:?}");
    assert_eq!(healed[0].0, ServiceKind::Redis);
    assert!(healed[0].1.is_ok(), "{healed:?}");
    assert_ne!(env.stack.redis().status().await.pid, before, "restarted");
    assert!(lumina_process::health::wait_tcp(env.cfg.redis.port, limit).await);

    env.stack.stop(ServiceKind::Redis).await.expect("stop");
}

#[tokio::test]
async fn redis_starts_serves_and_saves_on_stop() {
    let Some(env) = setup(&[("LUMINA_TEST_REDIS_DIR", "redis")]) else {
        return;
    };
    env.stack
        .start(ServiceKind::Redis, &env.cfg)
        .await
        .expect("start");

    let dir = PathBuf::from(std::env::var("LUMINA_TEST_REDIS_DIR").expect("var"));
    let cli = dir.join(format!("redis-cli{}", std::env::consts::EXE_SUFFIX));
    let port = env.cfg.redis.port.to_string();
    let run = |args: &[&str]| {
        let out = Command::new(&cli)
            .args(["-p", &port])
            .args(args)
            .output()
            .expect("cli");
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    };
    assert_eq!(run(&["PING"]), "PONG");
    assert_eq!(run(&["SET", "lumina", "ok"]), "OK");
    assert_eq!(run(&["GET", "lumina"]), "ok");

    env.stack.stop(ServiceKind::Redis).await.expect("stop");
    assert!(
        processes_under(&env.layout.bin().join("redis")).is_empty(),
        "leftovers"
    );
    assert!(
        env.layout.data().join("redis/dump.rdb").is_file(),
        "snapshot saved on stop"
    );
}

#[tokio::test]
async fn postgresql_initialises_serves_and_stops_cleanly() {
    let Some(env) = setup(&[("LUMINA_TEST_POSTGRES_DIR", "postgresql")]) else {
        return;
    };
    env.stack
        .start(ServiceKind::Postgresql, &env.cfg)
        .await
        .expect("start");
    let data = env
        .stack
        .postgresql()
        .status()
        .await
        .runtime
        .expect("runtime")
        .data_dir;
    assert!(data.join("PG_VERSION").is_file(), "initdb ran");

    let dir = PathBuf::from(std::env::var("LUMINA_TEST_POSTGRES_DIR").expect("var"));
    let psql = dir
        .join("bin")
        .join(format!("psql{}", std::env::consts::EXE_SUFFIX));
    let port = env.cfg.postgresql.port.to_string();
    let query = |sql: &str| {
        let out = Command::new(&psql)
            .args([
                "-h",
                "127.0.0.1",
                "-p",
                &port,
                "-U",
                "postgres",
                "-tAc",
                sql,
            ])
            .output()
            .expect("psql");
        assert!(out.status.success(), "psql: {out:?}");
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    };
    assert_eq!(query("SHOW server_encoding"), "UTF8");
    query("CREATE DATABASE lumina_test");
    let exists = "SELECT count(*) FROM pg_database WHERE datname = 'lumina_test'";
    assert_eq!(query(exists), "1");

    env.stack.stop(ServiceKind::Postgresql).await.expect("stop");
    assert!(
        processes_under(&env.layout.bin().join("postgresql")).is_empty(),
        "leftovers"
    );
    assert!(
        !data.join("postmaster.pid").exists(),
        "a clean shutdown removes postmaster.pid"
    );

    // A second start reuses the data folder.
    env.stack
        .start(ServiceKind::Postgresql, &env.cfg)
        .await
        .expect("restart");
    assert_eq!(query(exists), "1");
    env.stack.stop(ServiceKind::Postgresql).await.expect("stop");
}
