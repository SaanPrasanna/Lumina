//! End-to-end checks with real MySQL, MariaDB, Redis, PostgreSQL and PHP builds. Opt-in:
//!
//! ```powershell
//! $env:LUMINA_TEST_PHP_DIRS     = "C:\Lumina\bin\php\8.4.26"
//! $env:LUMINA_TEST_MYSQL_DIR    = "C:\Lumina\bin\mysql\8.4.11"
//! $env:LUMINA_TEST_MARIADB_DIR  = "C:\Lumina\bin\mariadb\12.3.3"
//! $env:LUMINA_TEST_REDIS_DIR    = "C:\Lumina\bin\redis\8.10.2"
//! $env:LUMINA_TEST_POSTGRES_DIR = "C:\Lumina\bin\postgresql\18.6"
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
