//! The runner with real processes: the system shell stands in for PHP.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use lumina_quickapp::runner::{self, Context, Event, Tool};
use lumina_quickapp::{Error, RecipeFile};

fn shell() -> Tool {
    if cfg!(windows) {
        Tool {
            program: "cmd.exe".into(),
            args: vec!["/d".into(), "/c".into()],
        }
    } else {
        Tool {
            program: "/bin/sh".into(),
            args: vec!["-c".into()],
        }
    }
}

fn context(root: &Path) -> Context {
    let templates = root.join("templates");
    std::fs::create_dir_all(templates.join("t")).expect("templates");
    std::fs::write(
        templates.join("t").join("hello.txt"),
        "hi {name} on {domain} {$php}\n",
    )
    .expect("template");
    Context {
        dir: root.join("www").join("demo"),
        www: root.join("www"),
        values: BTreeMap::from([
            ("name".to_owned(), "demo".to_owned()),
            ("domain".to_owned(), "demo.test".to_owned()),
            (
                "www".to_owned(),
                root.join("www").to_string_lossy().into_owned(),
            ),
            (
                "dir".to_owned(),
                root.join("www").join("demo").to_string_lossy().into_owned(),
            ),
        ]),
        tools: BTreeMap::from([("php".to_owned(), shell())]),
        env: vec![("LUMINA_TEST".into(), "1".into())],
        templates,
        work_dir: root.join("work"),
        proxy: String::new(),
    }
}

fn recipe(steps: &str) -> lumina_quickapp::Recipe {
    RecipeFile::parse(&format!(
        "[[recipe]]\nid = \"t\"\nname = \"T\"\ndescription = \"d\"\n{steps}"
    ))
    .expect("recipe")
    .recipes
    .remove(0)
}

fn collect() -> (Arc<Mutex<Vec<Event>>>, impl Fn(Event) + Send + Sync) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&events);
    (events, move |e| sink.lock().expect("events").push(e))
}

fn lines(events: &Mutex<Vec<Event>>) -> Vec<String> {
    events
        .lock()
        .expect("events")
        .iter()
        .filter_map(|e| match e {
            Event::Line { text } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// `mkdir <dir>` for the shell.
fn mkdir(dir: &str) -> String {
    // Unquoted on Windows: cmd.exe does not understand the `\"` escaping Rust
    // uses for arguments (temp paths have no spaces).
    if cfg!(windows) {
        format!("mkdir {dir}")
    } else {
        format!("mkdir -p '{dir}'")
    }
}

#[tokio::test]
async fn steps_run_in_order_and_files_are_written() {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(tmp.path().join("www")).expect("www");
    let ctx = context(tmp.path());
    let steps = format!(
        r#"
[[recipe.step]]
label = "make"
type = "run"
program = "php"
cwd = "{{www}}"
args = ["{mk} && echo made it"]

[[recipe.step]]
label = "env"
type = "patch_env"
file = ".env"
set = {{ APP_URL = "https://{{domain}}" }}

[[recipe.step]]
label = "tpl"
type = "template_file"
template = "t/hello.txt"
to = "sub/hello.txt"
"#,
        mk = mkdir("{dir}")
    );
    let recipe = recipe(&steps);
    let (events, emit) = collect();
    let report = runner::run(
        &recipe,
        &BTreeMap::new(),
        &ctx,
        &Arc::new(AtomicBool::new(false)),
        &emit,
    )
    .await;
    report.result.expect("run");
    assert!(
        lines(&events).iter().any(|l| l == "made it"),
        "{:?}",
        lines(&events)
    );
    assert_eq!(
        std::fs::read_to_string(ctx.dir.join(".env")).expect(".env"),
        "APP_URL=https://demo.test\n"
    );
    assert_eq!(
        std::fs::read_to_string(ctx.dir.join("sub").join("hello.txt")).expect("tpl"),
        "hi demo on demo.test {$php}\n"
    );
    let steps: Vec<_> = events
        .lock()
        .expect("events")
        .iter()
        .filter_map(|e| match e {
            Event::Step { index, total, .. } => Some((*index, *total)),
            _ => None,
        })
        .collect();
    assert_eq!(steps, vec![(1, 3), (2, 3), (3, 3)]);
}

#[tokio::test]
async fn a_failing_command_reports_its_exit_code() {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(tmp.path().join("www")).expect("www");
    let ctx = context(tmp.path());
    let exit = if cfg!(windows) { "exit /b 3" } else { "exit 3" };
    let recipe = recipe(&format!(
        "[[recipe.step]]\nlabel = \"x\"\ntype = \"run\"\nprogram = \"php\"\nargs = [\"echo before && {exit}\"]\n"
    ));
    let (events, emit) = collect();
    let report = runner::run(
        &recipe,
        &BTreeMap::new(),
        &ctx,
        &Arc::new(AtomicBool::new(false)),
        &emit,
    )
    .await;
    match report.result {
        Err(Error::Command { code: Some(3), .. }) => {}
        other => panic!("{other:?}"),
    }
    assert!(lines(&events).iter().any(|l| l == "before"));
}

#[tokio::test]
async fn a_fallback_starts_from_a_clean_folder() {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(tmp.path().join("www")).expect("www");
    let ctx = context(tmp.path());
    let fail = if cfg!(windows) { "exit /b 1" } else { "exit 1" };
    let mk = mkdir("{dir}");
    let marker = if cfg!(windows) {
        "echo x> {dir}\\\\half-done"
    } else {
        "touch '{dir}/half-done'"
    };
    let recipe = recipe(&format!(
        r#"
[[recipe.step]]
label = "primary"
type = "run"
program = "php"
cwd = "{{www}}"
args = ["{mk} && {marker} && {fail}"]

  [[recipe.step.fallback]]
  label = "alternative"
  type = "run"
  program = "php"
  cwd = "{{www}}"
  args = ["{mk}"]

[[recipe.step]]
label = "optional failure"
type = "run"
program = "php"
optional = true
args = ["{fail}"]
"#
    ));
    let (events, emit) = collect();
    let report = runner::run(
        &recipe,
        &BTreeMap::new(),
        &ctx,
        &Arc::new(AtomicBool::new(false)),
        &emit,
    )
    .await;
    report.result.expect("the fallback succeeds");
    assert!(ctx.dir.is_dir());
    assert!(
        !ctx.dir.join("half-done").exists(),
        "the failed attempt was cleaned up"
    );
    assert!(lines(&events).iter().any(|l| l.contains("continuing")));
}

#[tokio::test]
async fn an_existing_project_folder_is_refused() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let ctx = context(tmp.path());
    std::fs::create_dir_all(&ctx.dir).expect("dir");
    let recipe = recipe(
        "[[recipe.step]]\nlabel = \"x\"\ntype = \"run\"\nprogram = \"php\"\nargs = [\"echo\"]\n",
    );
    let (_, emit) = collect();
    let report = runner::run(
        &recipe,
        &BTreeMap::new(),
        &ctx,
        &Arc::new(AtomicBool::new(false)),
        &emit,
    )
    .await;
    assert!(report.result.is_err());
}

/// Cancelling kills the command and the processes it started.
#[tokio::test]
async fn cancel_kills_the_whole_tree() {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(tmp.path().join("www")).expect("www");
    let mut ctx = context(tmp.path());
    let pid_file = tmp.path().join("child.pid");
    // A shell that starts a long-running grandchild, records its PID and waits.
    let script = if cfg!(windows) {
        ctx.tools.insert(
            "php".into(),
            Tool {
                program: "powershell.exe".into(),
                args: vec!["-NoProfile".into(), "-Command".into()],
            },
        );
        format!(
            "$p = Start-Process -FilePath ping.exe -ArgumentList '-n','60','127.0.0.1' -PassThru -WindowStyle Hidden; $p.Id | Out-File -Encoding ascii '{}'; Wait-Process -Id $p.Id",
            pid_file.display()
        )
    } else {
        format!("sleep 60 & echo $! > '{}'; wait", pid_file.display())
    };
    let recipe = recipe(&format!(
        "[[recipe.step]]\nlabel = \"long\"\ntype = \"run\"\nprogram = \"php\"\ncwd = \"{{www}}\"\nargs = ['''{script}''']\n"
    ));
    let cancel = Arc::new(AtomicBool::new(false));
    let (_, emit) = collect();
    let started = Instant::now();
    let options = BTreeMap::new();
    let run = runner::run(&recipe, &options, &ctx, &cancel, &emit);
    let trigger = async {
        // Wait until the grandchild is up, then cancel.
        let deadline = Instant::now() + Duration::from_secs(30);
        while !pid_file.is_file()
            || std::fs::read_to_string(&pid_file)
                .unwrap_or_default()
                .trim()
                .is_empty()
        {
            assert!(Instant::now() < deadline, "the grandchild never started");
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        cancel.store(true, Ordering::Relaxed);
    };
    let (report, ()) = tokio::join!(run, trigger);
    assert!(
        matches!(report.result, Err(Error::Cancelled)),
        "{:?}",
        report.result
    );
    assert!(started.elapsed() < Duration::from_secs(40));

    let pid: u32 = std::fs::read_to_string(&pid_file)
        .expect("pid")
        .trim()
        .trim_start_matches('\u{feff}')
        .parse()
        .expect("pid number");
    // A killed process can stay listed for a moment while Windows tears it
    // down (seen on a busy CI runner). One that escaped the kill would still
    // run its 60-second ping, so a few seconds of grace cannot hide that.
    // Matching the name also ignores a reused PID.
    let alive = |system: &mut sysinfo::System| {
        system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        system
            .process(sysinfo::Pid::from_u32(pid))
            .is_some_and(|p| {
                let name = p.name().to_string_lossy().to_ascii_lowercase();
                name.starts_with("ping") || name.starts_with("sleep")
            })
    };
    let mut system = sysinfo::System::new();
    let deadline = Instant::now() + Duration::from_secs(5);
    while alive(&mut system) {
        assert!(
            Instant::now() < deadline,
            "grandchild {pid} survived the cancel"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
