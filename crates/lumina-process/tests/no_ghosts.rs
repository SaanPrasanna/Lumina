//! The core Phase 1 guarantee: nothing Lumina starts survives a stop, a restart,
//! a version switch or the supervisor going away.
//!
//! Each test copies the `lumina-fake-fcgi` fixture to its own temp path, so
//! "processes still alive" can be counted by executable path without seeing the
//! processes of tests running in parallel.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use lumina_process::ledger::Ledger;
use lumina_process::{ProcessEvent, ProcessSpec, RestartPolicy, Supervisor};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
use tempfile::TempDir;
use tokio::time::{Instant, sleep};

const STOP_LIMIT: Duration = Duration::from_secs(5);

struct Fixture {
    _tmp: TempDir,
    exe: PathBuf,
    logs: PathBuf,
}

/// A private copy of the fake server inside `<tmp>/bin/`.
fn fixture(name: &str) -> Fixture {
    let tmp = tempfile::tempdir().expect("tempdir");
    let bin = tmp.path().join("bin");
    std::fs::create_dir_all(&bin).expect("bin dir");
    let exe = bin.join(format!("fake-{name}{}", std::env::consts::EXE_SUFFIX));
    std::fs::copy(env!("CARGO_BIN_EXE_lumina-fake-fcgi"), &exe).expect("copy fixture");
    let logs = tmp.path().join("logs");
    Fixture {
        _tmp: tmp,
        exe,
        logs,
    }
}

impl Fixture {
    fn spec(&self, id: &str, env: &[(&str, &str)]) -> ProcessSpec {
        ProcessSpec {
            id: id.to_owned(),
            program: self.exe.clone(),
            args: Vec::new(),
            env: env
                .iter()
                .map(|(k, v)| (OsString::from(k), OsString::from(v)))
                .collect(),
            cwd: None,
            log_file: self.logs.join(format!("{id}.log")),
            restart: RestartPolicy::Never,
        }
    }

    fn bin_dir(&self) -> &Path {
        self.exe.parent().expect("bin dir")
    }
}

/// PIDs of live processes running `exe`.
fn alive_with_exe(exe: &Path) -> Vec<u32> {
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_exe(UpdateKind::Always),
    );
    sys.processes()
        .iter()
        .filter(|(_, p)| p.exe() == Some(exe))
        .map(|(pid, _)| pid.as_u32())
        .collect()
}

/// Waits until `id`'s group contains exactly `n` live processes running the
/// fixture. Groups can hold extra processes the OS adds (on Windows every console
/// program gets a `conhost.exe` in the same job); those are not counted here but
/// must still die on stop, which `assert_none_alive` and `stop` verify.
async fn wait_members(sup: &Supervisor, id: &str, exe: &Path, n: usize) -> Vec<u32> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let group = sup.members(id).await;
        let ours: Vec<u32> = alive_with_exe(exe)
            .into_iter()
            .filter(|pid| group.contains(pid))
            .collect();
        if ours.len() == n {
            return ours;
        }
        assert!(
            Instant::now() < deadline,
            "expected {n} fixture processes, have {ours:?} (group: {group:?})"
        );
        sleep(Duration::from_millis(25)).await;
    }
}

/// Waits until no process runs `exe` (the OS may need a moment to reap).
async fn assert_none_alive(exe: &Path) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let alive = alive_with_exe(exe);
        if alive.is_empty() {
            return;
        }
        assert!(Instant::now() < deadline, "ghost processes: {alive:?}");
        sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn stop_kills_the_leader_and_all_workers() {
    let fx = fixture("stop");
    let sup = Supervisor::new(Ledger::disabled());

    sup.start(fx.spec("php-fcgi", &[("FAKE_CHILDREN", "4")]))
        .await
        .expect("start");
    wait_members(&sup, "php-fcgi", &fx.exe, 5).await;
    assert_eq!(alive_with_exe(&fx.exe).len(), 5);

    sup.stop("php-fcgi", STOP_LIMIT).await.expect("stop");

    assert_none_alive(&fx.exe).await;
    assert!(sup.info("php-fcgi").await.is_none());
}

#[tokio::test]
async fn version_switch_leaves_no_old_processes() {
    let old = fixture("php83");
    let new = fixture("php84");
    let sup = Supervisor::new(Ledger::disabled());

    sup.start(old.spec("php-fcgi", &[("FAKE_CHILDREN", "3")]))
        .await
        .expect("start old");
    wait_members(&sup, "php-fcgi", &old.exe, 4).await;

    sup.stop("php-fcgi", STOP_LIMIT).await.expect("stop old");
    sup.start(new.spec("php-fcgi", &[("FAKE_CHILDREN", "3")]))
        .await
        .expect("start new");
    wait_members(&sup, "php-fcgi", &new.exe, 4).await;

    assert_none_alive(&old.exe).await;
    assert_eq!(alive_with_exe(&new.exe).len(), 4);
    sup.stop("php-fcgi", STOP_LIMIT).await.expect("stop new");
    assert_none_alive(&new.exe).await;
}

#[test]
fn supervisor_going_away_kills_the_tree() {
    let fx = fixture("crash");
    // Simulates Lumina dying without calling stop: the runtime and the supervisor
    // are dropped while the server runs.
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let sup = Supervisor::new(Ledger::disabled());
    rt.block_on(async {
        sup.start(fx.spec("php-fcgi", &[("FAKE_CHILDREN", "3")]))
            .await
            .expect("start");
        wait_members(&sup, "php-fcgi", &fx.exe, 4).await;
    });

    drop(sup);
    drop(rt);

    let check = tokio::runtime::Runtime::new().expect("runtime");
    check.block_on(assert_none_alive(&fx.exe));
}

#[tokio::test]
async fn crashing_server_is_restarted_then_given_up_without_leftovers() {
    let fx = fixture("restart");
    let sup = Supervisor::new(Ledger::disabled());
    let mut events = sup.subscribe();
    let mut spec = fx.spec(
        "php-fcgi",
        &[("FAKE_CHILDREN", "2"), ("FAKE_EXIT_AFTER_MS", "300")],
    );
    spec.restart = RestartPolicy::OnFailure {
        max_restarts: 2,
        backoff: Duration::from_millis(50),
    };

    sup.start(spec).await.expect("start");

    let mut restarts = 0;
    let reason = loop {
        let event = tokio::time::timeout(Duration::from_secs(15), events.recv())
            .await
            .expect("event in time")
            .expect("channel open");
        match event {
            ProcessEvent::Restarting { attempt, .. } => {
                restarts += 1;
                assert_eq!(attempt, restarts);
            }
            ProcessEvent::Failed { reason, .. } => break reason,
            _ => {}
        }
    };

    assert_eq!(restarts, 2);
    assert!(reason.contains("gave up"), "{reason}");
    // Workers of every crashed generation were killed along with their leader.
    assert_none_alive(&fx.exe).await;

    // A failed process can be started again.
    sup.stop("php-fcgi", STOP_LIMIT)
        .await
        .expect("clear failed");
    sup.start(fx.spec("php-fcgi", &[]))
        .await
        .expect("start again");
    sup.stop("php-fcgi", STOP_LIMIT).await.expect("stop");
}

#[tokio::test]
async fn hundred_start_stop_cycles_leave_nothing_behind() {
    let fx = fixture("cycles");
    let sup = Supervisor::new(Ledger::disabled());

    for i in 0..100 {
        sup.start(fx.spec("php-fcgi", &[("FAKE_CHILDREN", "2")]))
            .await
            .unwrap_or_else(|e| panic!("start #{i}: {e}"));
        // Stop at varying points of start-up to shake out races.
        if i % 3 == 0 {
            wait_members(&sup, "php-fcgi", &fx.exe, 3).await;
        }
        sup.stop("php-fcgi", STOP_LIMIT)
            .await
            .unwrap_or_else(|e| panic!("stop #{i}: {e}"));
    }

    assert_none_alive(&fx.exe).await;
}

#[tokio::test]
async fn ledger_sweep_kills_leftovers_from_a_crashed_run() {
    let fx = fixture("sweep");
    let ledger_path = fx.logs.with_file_name("pids.json");

    // A previous run recorded a server and then died without stopping it.
    let mut leftover = std::process::Command::new(&fx.exe)
        .spawn()
        .expect("spawn leftover");
    Ledger::open(&ledger_path).record("php-fcgi", leftover.id(), &fx.exe);

    let swept = Ledger::open(&ledger_path).sweep(fx.bin_dir());

    assert_eq!(swept.len(), 1);
    assert_eq!(swept[0].pid, leftover.id());
    let status = leftover.wait().expect("reaped");
    assert!(!status.success());
    assert!(Ledger::open(&ledger_path).sweep(fx.bin_dir()).is_empty());
}

#[tokio::test]
async fn logs_capture_server_output() {
    let fx = fixture("logs");
    let sup = Supervisor::new(Ledger::disabled());
    let spec = fx.spec("php-fcgi", &[("FAKE_CHILDREN", "1")]);
    let log = spec.log_file.clone();

    sup.start(spec).await.expect("start");
    wait_members(&sup, "php-fcgi", &fx.exe, 2).await;
    sup.stop("php-fcgi", STOP_LIMIT).await.expect("stop");

    let text = lumina_process::logs::tail(&log, 10);
    assert!(text.contains("Lumina:"), "{text}");
    assert!(text.contains("with 1 workers"), "{text}");
}
