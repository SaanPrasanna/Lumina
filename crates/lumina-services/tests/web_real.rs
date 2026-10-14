//! End-to-end checks with real nginx, Apache and PHP builds. Opt-in:
//!
//! ```powershell
//! $env:LUMINA_TEST_PHP_DIRS   = "C:\Lumina\bin\php\8.4.26"
//! $env:LUMINA_TEST_NGINX_DIR  = "C:\Lumina\bin\nginx\1.30.5"
//! $env:LUMINA_TEST_APACHE_DIR = "C:\Lumina\bin\apache\2.4.69"
//! cargo test -p lumina-services --test web_real -- --nocapture --test-threads=1
//! ```
//!
//! Everything is copied into a temporary data root; the real one is untouched.
//! Run one test at a time: each copies the server builds, and parallel copies
//! plus antivirus scans of the fresh executables make start-up slow.
//! Without the variables every test returns early.

use std::path::{Path, PathBuf};
use std::time::Duration;

use lumina_config::AppConfig;
use lumina_config::schema::WebServer;
use lumina_core::Error;
use lumina_core::paths::Layout;
use lumina_core::types::ServiceKind;
use lumina_process::Supervisor;
use lumina_process::ledger::Ledger;
use lumina_services::orchestrator::Orchestrator;
use lumina_services::template::Templates;
use lumina_services::web::ReloadOutcome;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
use tempfile::TempDir;

struct Env {
    _tmp: TempDir,
    layout: Layout,
    stack: Orchestrator,
    cfg: AppConfig,
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

/// A data root with copies of the configured builds, and a config on free ports.
fn setup(server: WebServer) -> Option<Env> {
    let php_dirs = std::env::var("LUMINA_TEST_PHP_DIRS").ok()?;
    let nginx = std::env::var("LUMINA_TEST_NGINX_DIR").ok();
    let apache = std::env::var("LUMINA_TEST_APACHE_DIR").ok();
    if nginx.is_none() && server == WebServer::Nginx
        || apache.is_none() && server == WebServer::Apache
    {
        return None;
    }

    let tmp = tempfile::tempdir().expect("tempdir");
    let layout = Layout::new(tmp.path().join("Lumina"));
    layout.ensure().expect("layout");
    for dir in std::env::split_paths(&php_dirs) {
        copy_dir(
            &dir,
            &layout
                .bin()
                .join("php")
                .join(dir.file_name().expect("name")),
        );
    }
    for (tool, dir) in [("nginx", nginx), ("apache", apache)] {
        if let Some(dir) = dir.map(PathBuf::from) {
            copy_dir(
                &dir,
                &layout.bin().join(tool).join(dir.file_name().expect("name")),
            );
        }
    }

    let mut cfg = AppConfig::default();
    cfg.web.server = server;
    cfg.web.http_port = free_port();
    cfg.php.fcgi_port = free_port();
    cfg.php.fcgi_fallback_port = free_port();
    cfg.php.fcgi_children = 4;

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

impl Env {
    fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.cfg.web.http_port)
    }

    fn web_kind(&self) -> ServiceKind {
        Orchestrator::configured_web(&self.cfg)
    }

    /// PHP and the configured web server. Not `start_all`: that also starts
    /// the database, which these tests do not install.
    async fn start_web(&self) -> lumina_core::Result<()> {
        self.stack.start(self.web_kind(), &self.cfg).await
    }

    /// Tails of the server and PHP logs, for assertion messages.
    fn logs(&self) -> String {
        [
            "apache_error.log",
            "nginx_error.log",
            "php_errors.log",
            "php-fcgi.log",
        ]
        .iter()
        .map(|name| {
            let tail = lumina_process::logs::tail(&self.layout.logs().join(name), 25);
            format!(
                "--- {name}
{tail}"
            )
        })
        .collect::<Vec<_>>()
        .join(
            "
",
        )
    }

    fn write_www(&self, name: &str, body: &str) {
        std::fs::write(self.layout.www().join(name), body).expect("write www");
    }
}

/// Processes whose executable lives under `root`.
fn processes_under(root: &Path) -> Vec<(u32, String)> {
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_exe(UpdateKind::Always),
    );
    sys.processes()
        .iter()
        .filter_map(|(pid, p)| {
            let exe = p.exe()?;
            exe.starts_with(root)
                .then(|| (pid.as_u32(), exe.display().to_string()))
        })
        .collect()
}

async fn get(url: &str) -> (u16, String) {
    let resp = reqwest::get(url).await.expect("request");
    let status = resp.status().as_u16();
    (status, resp.text().await.expect("body"))
}

async fn serves_static_and_php(server: WebServer) {
    let Some(env) = setup(server) else { return };
    env.start_web().await.expect("start stack");
    env.write_www("phpinfo.php", "<?php phpinfo();");

    let (status, body) = get(&env.url("/")).await;
    assert_eq!(status, 200);
    assert!(body.contains("Lumina is running"), "landing page: {body}");

    let (status, body) = get(&env.url("/phpinfo.php")).await;
    assert!(
        body.contains("PHP Version"),
        "phpinfo ({status}): {}
{}",
        &body[..body.len().min(300)],
        env.logs()
    );

    let failures = env.stack.stop_all().await;
    assert!(failures.is_empty(), "{failures:?}");
    assert!(
        processes_under(&env.layout.bin()).is_empty(),
        "leftovers after stop_all"
    );
}

#[tokio::test]
async fn nginx_serves_static_and_php() {
    serves_static_and_php(WebServer::Nginx).await;
}

#[tokio::test]
async fn apache_serves_static_and_php() {
    serves_static_and_php(WebServer::Apache).await;
}

/// GET `path` on the test port with `Host: host`, the way a browser asks for
/// http://<host>/ once the hosts file points the name at 127.0.0.1.
async fn get_host(env: &Env, host: &str, path: &str) -> (u16, String) {
    let resp = reqwest::Client::new()
        .get(env.url(path))
        .header("Host", host)
        .send()
        .await
        .expect("request");
    let status = resp.status().as_u16();
    (status, resp.text().await.expect("body"))
}

/// Each project folder becomes `<folder>.test` with its detected docroot;
/// adding and removing folders takes effect with one sync.
async fn auto_vhosts_follow_the_projects_folder(server: WebServer) {
    let Some(env) = setup(server) else { return };
    let www = env.layout.www();
    std::fs::create_dir_all(www.join("blog").join("public")).expect("mkdir");
    std::fs::write(
        www.join("blog").join("public").join("index.php"),
        r#"<?php echo "blog:" . $_SERVER['HTTP_HOST'] . ":" . basename(__DIR__);"#,
    )
    .expect("write");
    env.start_web().await.expect("start stack");

    let (status, body) = get_host(&env, "blog.test", "/").await;
    assert_eq!(
        (status, body.as_str()),
        (200, "blog:blog.test:public"),
        "{}",
        env.logs()
    );
    // Front controller: unknown paths go to index.php.
    let (_, body) = get_host(&env, "blog.test", "/posts/1").await;
    assert_eq!(body, "blog:blog.test:public");
    // localhost still serves the projects folder.
    let (_, body) = get(&env.url("/")).await;
    assert!(body.contains("Lumina is running"), "{body}");

    // A new folder is live after one sync.
    std::fs::create_dir_all(www.join("Shop")).expect("mkdir");
    std::fs::write(www.join("Shop").join("index.html"), "shop home").expect("write");
    let started = std::time::Instant::now();
    let outcome = env.stack.sync_sites(&env.cfg).await.expect("sync");
    assert!(
        matches!(
            outcome,
            Some(ReloadOutcome::Reloaded { .. } | ReloadOutcome::Restarted)
        ),
        "{outcome:?}"
    );
    let (status, body) = get_host(&env, "shop.test", "/").await;
    assert_eq!((status, body.as_str()), (200, "shop home"));
    // nginx reloads in place; Apache on Windows has no graceful reload outside
    // service mode and is restarted, which takes a few seconds more.
    let limit = match server {
        WebServer::Nginx => Duration::from_secs(3),
        WebServer::Apache => Duration::from_secs(10),
    };
    let elapsed = started.elapsed();
    println!("{server:?}: shop.test live {elapsed:?} after the sync started");
    assert!(elapsed < limit, "shop.test live after {elapsed:?}");

    // Nothing changed → no reload.
    assert_eq!(
        env.stack.sync_sites(&env.cfg).await.expect("noop"),
        Some(ReloadOutcome::Unchanged)
    );

    // A removed folder loses its vhost: the name falls through to localhost.
    std::fs::remove_dir_all(www.join("blog")).expect("rm");
    env.stack.sync_sites(&env.cfg).await.expect("sync");
    let (_, body) = get_host(&env, "blog.test", "/").await;
    assert!(!body.starts_with("blog:"), "{body}");
    let sites_dir = match server {
        WebServer::Nginx => env.stack.nginx().sites_dir(),
        WebServer::Apache => env.stack.apache().sites_dir(),
    };
    assert!(!sites_dir.join("auto.blog.test.conf").exists());
    assert!(sites_dir.join("auto.shop.test.conf").exists());

    let failures = env.stack.stop_all().await;
    assert!(failures.is_empty(), "{failures:?}");
}

#[tokio::test]
async fn nginx_auto_vhosts_follow_the_projects_folder() {
    auto_vhosts_follow_the_projects_folder(WebServer::Nginx).await;
}

#[tokio::test]
async fn apache_auto_vhosts_follow_the_projects_folder() {
    auto_vhosts_follow_the_projects_folder(WebServer::Apache).await;
}

#[tokio::test]
async fn nginx_reload_takes_exactly_one_signal_and_is_live_quickly() {
    let Some(mut env) = setup(WebServer::Nginx) else {
        return;
    };
    env.start_web().await.expect("start");
    let (_, before) = get(&env.url("/")).await;
    assert!(before.contains("Lumina is running"));

    // Nothing changed → no signal at all.
    let outcome = env
        .stack
        .reload(ServiceKind::Nginx, &env.cfg)
        .await
        .expect("noop");
    assert_eq!(outcome, ReloadOutcome::Unchanged);

    // Alternate localhost between two folders; every switch must go live
    // after exactly one reload signal.
    let other = env.layout.root().join("other-www");
    std::fs::create_dir_all(&other).expect("mkdir");
    std::fs::write(other.join("index.html"), "second root").expect("write");

    for round in 0..6 {
        let use_other = round % 2 == 0;
        env.cfg.general.www_root = use_other.then(|| other.clone());
        let started = std::time::Instant::now();

        let outcome = env
            .stack
            .reload(ServiceKind::Nginx, &env.cfg)
            .await
            .expect("reload");

        assert_eq!(
            outcome,
            ReloadOutcome::Reloaded { signals: 1 },
            "round {round}"
        );
        let (_, body) = get(&env.url("/")).await;
        if use_other {
            assert_eq!(body, "second root", "round {round}");
        } else {
            assert!(body.contains("Lumina is running"), "round {round}: {body}");
        }
        let elapsed = started.elapsed();
        assert!(
            elapsed < Duration::from_secs(2),
            "round {round}: live after {elapsed:?}"
        );
    }
    env.stack.stop_all().await;
}

#[tokio::test]
async fn broken_config_is_rejected_rolled_back_and_the_site_keeps_working() {
    let Some(mut env) = setup(WebServer::Nginx) else {
        return;
    };
    env.start_web().await.expect("start");
    let site = env.stack.nginx().sites_dir().join("00-localhost.conf");
    let good = std::fs::read_to_string(&site).expect("site");

    // A user's own site file with a syntax error makes `nginx -t` fail.
    std::fs::write(
        env.stack.nginx().sites_dir().join("mine.conf"),
        "server { listen ;",
    )
    .expect("w");
    env.cfg.general.www_root = Some(env.layout.root().join("elsewhere"));
    let err = env
        .stack
        .reload(ServiceKind::Nginx, &env.cfg)
        .await
        .expect_err("bad config");
    assert!(
        err.to_string().contains("configuration test failed"),
        "{err}"
    );

    assert_eq!(
        std::fs::read_to_string(&site).expect("site"),
        good,
        "rolled back"
    );
    let (status, _) = get(&env.url("/")).await;
    assert_eq!(status, 200, "old config still serving");
    env.stack.stop_all().await;
}

/// Blueprint §2: PHP workers recycle every 50 requests; clients must not notice.
async fn no_5xx_under_keepalive_load(server: WebServer) {
    let Some(mut env) = setup(server) else { return };
    env.cfg.php.fcgi_max_requests = 50;
    env.start_web().await.expect("start");
    env.write_www("hello.php", "<?php echo 'ok';");

    let client = reqwest::Client::builder()
        .pool_max_idle_per_host(8)
        // A hung request must fail the test, not stall it.
        .timeout(Duration::from_secs(30))
        .build()
        .expect("client");
    let url = env.url("/hello.php");
    let mut tasks = Vec::new();
    for _ in 0..8 {
        let client = client.clone();
        let url = url.clone();
        tasks.push(tokio::spawn(async move {
            let mut bad = Vec::new();
            for _ in 0..250 {
                match client.get(&url).send().await {
                    Ok(r) if r.status().is_success() => {}
                    Ok(r) => bad.push(r.status().as_u16().to_string()),
                    Err(e) => bad.push(e.to_string()),
                }
            }
            bad
        }));
    }
    let mut bad = Vec::new();
    for task in tasks {
        bad.extend(task.await.expect("task"));
    }
    println!("{server:?}: 2000 requests, {} failures", bad.len());
    assert!(
        bad.is_empty(),
        "{} failed requests, e.g. {:?}",
        bad.len(),
        &bad[..bad.len().min(5)]
    );
    env.stack.stop_all().await;
}

#[tokio::test]
async fn nginx_has_no_5xx_while_php_workers_recycle() {
    no_5xx_under_keepalive_load(WebServer::Nginx).await;
}

#[tokio::test]
async fn apache_has_no_5xx_while_php_workers_recycle() {
    no_5xx_under_keepalive_load(WebServer::Apache).await;
}

#[tokio::test]
async fn busy_http_port_names_the_owner() {
    let Some(env) = setup(WebServer::Nginx) else {
        return;
    };
    let _blocker = std::net::TcpListener::bind(("127.0.0.1", env.cfg.web.http_port)).expect("bind");

    let err = env
        .stack
        .start(env.web_kind(), &env.cfg)
        .await
        .expect_err("port busy");

    match err {
        Error::PortInUse { port, owner } => {
            assert_eq!(port, env.cfg.web.http_port);
            let owner = owner.expect("owner");
            assert!(owner.contains(&std::process::id().to_string()), "{owner}");
        }
        other => panic!("expected PortInUse, got {other}"),
    }
    env.stack.stop_all().await;
}

#[tokio::test]
async fn starting_one_web_server_stops_the_other() {
    if std::env::var("LUMINA_TEST_APACHE_DIR").is_err() {
        return;
    }
    let Some(mut env) = setup(WebServer::Nginx) else {
        return;
    };
    env.start_web().await.expect("nginx");
    let nginx_root = env.layout.bin().join("nginx");
    assert!(!processes_under(&nginx_root).is_empty());

    env.cfg.web.server = WebServer::Apache;
    env.stack
        .start(ServiceKind::Apache, &env.cfg)
        .await
        .expect("apache");

    assert!(
        processes_under(&nginx_root).is_empty(),
        "nginx still running"
    );
    let (status, _) = get(&env.url("/")).await;
    assert_eq!(status, 200);
    let status = env.stack.status(&env.cfg).await;
    assert_eq!(status.web.server, WebServer::Apache);
    env.stack.stop_all().await;
    assert!(processes_under(&env.layout.bin()).is_empty());
}
