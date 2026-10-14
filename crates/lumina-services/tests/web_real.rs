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
    // HTTPS is on by default; keep it off 443 so tests never collide.
    cfg.web.https_port = free_port();
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

/// A client that trusts only Lumina's CA and resolves `hosts` to 127.0.0.1,
/// like a browser after the CA is trusted and the hosts file is updated.
/// rustls/webpki also enforces the CA's name constraints.
fn tls_client(env: &Env, hosts: &[&str]) -> reqwest::Client {
    let ca = std::fs::read(env.layout.ssl().join("ca.crt")).expect("ca.crt");
    let mut builder = reqwest::Client::builder()
        .add_root_certificate(reqwest::Certificate::from_pem(&ca).expect("pem"))
        .redirect(reqwest::redirect::Policy::none());
    for host in hosts {
        for port in [env.cfg.web.http_port, env.cfg.web.https_port] {
            builder = builder.resolve(host, std::net::SocketAddr::from(([127, 0, 0, 1], port)));
        }
    }
    builder.build().expect("client")
}

/// https://blog.test and https://localhost are served with certificates from
/// the local CA; "Force HTTPS" redirects plain HTTP; turning HTTPS off goes
/// back to HTTP only.
async fn https_with_the_local_ca(server: WebServer) {
    let Some(mut env) = setup(server) else { return };
    let www = env.layout.www();
    std::fs::create_dir_all(www.join("blog").join("public")).expect("mkdir");
    std::fs::write(
        www.join("blog").join("public").join("index.php"),
        r#"<?php echo "secure:" . (($_SERVER['HTTPS'] ?? '') === 'on' ? 'yes' : 'no');"#,
    )
    .expect("write");
    env.start_web().await.expect("start");

    let https = env.cfg.web.https_port;
    let client = tls_client(&env, &["blog.test", "localhost"]);
    let get = |url: String| {
        let client = client.clone();
        async move {
            let resp = client
                .get(&url)
                .send()
                .await
                .unwrap_or_else(|e| panic!("{url}: {e:?}"));
            let status = resp.status().as_u16();
            let location = resp
                .headers()
                .get("location")
                .map(|v| v.to_str().unwrap_or_default().to_owned());
            (status, location, resp.text().await.unwrap_or_default())
        }
    };

    let (status, _, body) = get(format!("https://blog.test:{https}/")).await;
    assert_eq!(
        (status, body.as_str()),
        (200, "secure:yes"),
        "{}",
        env.logs()
    );
    let (status, _, body) = get(format!("https://localhost:{https}/")).await;
    assert_eq!(status, 200);
    assert!(body.contains("Lumina is running"));
    // Plain HTTP still works while "Force HTTPS" is off.
    let (_, _, body) = get(format!("http://blog.test:{}/", env.cfg.web.http_port)).await;
    assert_eq!(body, "secure:no");

    env.cfg.general.force_https = true;
    env.stack.sync_sites(&env.cfg).await.expect("force https");
    let (status, location, _) = get(format!(
        "http://blog.test:{}/posts?x=1",
        env.cfg.web.http_port
    ))
    .await;
    assert_eq!(status, 301);
    assert_eq!(
        location.as_deref(),
        Some(format!("https://blog.test:{https}/posts?x=1").as_str())
    );

    env.cfg.general.auto_ssl = false;
    env.stack.sync_sites(&env.cfg).await.expect("https off");
    let refused = client
        .get(format!("https://blog.test:{https}/"))
        .send()
        .await;
    assert!(refused.is_err(), "HTTPS should be off");
    let (status, _, body) = get(format!("http://blog.test:{}/", env.cfg.web.http_port)).await;
    assert_eq!((status, body.as_str()), (200, "secure:no"));

    let failures = env.stack.stop_all().await;
    assert!(failures.is_empty(), "{failures:?}");
}

#[tokio::test]
async fn nginx_serves_https_with_the_local_ca() {
    https_with_the_local_ca(WebServer::Nginx).await;
}

#[tokio::test]
async fn apache_serves_https_with_the_local_ca() {
    https_with_the_local_ca(WebServer::Apache).await;
}

/// A stand-in phpMyAdmin: `index.php` prints its name and the PHP version;
/// `.lumina-package.json` carries the PHP requirement, as Quick Add writes it.
fn fake_db_ui(env: &Env, version: &str, requires_php: &str) {
    let dir = env.layout.bin().join("phpmyadmin").join(version);
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(
        dir.join("index.php"),
        format!(r#"<?php echo "pma {version} on " . PHP_MAJOR_VERSION . "." . PHP_MINOR_VERSION;"#),
    )
    .expect("index");
    std::fs::write(
        dir.join(".lumina-package.json"),
        format!(
            r#"{{"package":"phpmyadmin","version":"{version}","url":"https://x","sha256":"00","installedAt":0,"requires":{{"php":"{requires_php}"}}}}"#
        ),
    )
    .expect("meta");
}

/// With PHP 8.3 active the PHP-8.3-capable phpMyAdmin is served at
/// /phpmyadmin/, with PHP 8.4 the one that needs 8.4 (blueprint §5).
async fn db_ui_follows_the_active_php(server: WebServer) {
    let Some(mut env) = setup(server) else { return };
    let phps = std::env::var("LUMINA_TEST_PHP_DIRS").unwrap_or_default();
    if !(phps.contains("8.3") && phps.contains("8.4")) {
        eprintln!("needs PHP 8.3 and 8.4 in LUMINA_TEST_PHP_DIRS");
        return;
    }
    fake_db_ui(&env, "6.0.0", ">=8.4");
    fake_db_ui(&env, "5.2.3", ">=7.2.5, <8.4");

    env.cfg.php.version = "8.3".into();
    env.start_web().await.expect("start");
    let (status, body) = get(&env.url("/phpmyadmin/")).await;
    assert_eq!(
        (status, body.as_str()),
        (200, "pma 5.2.3 on 8.3"),
        "{}",
        env.logs()
    );
    let (status, _) = get(&env.url("/phpmyadmin")).await;
    assert_eq!(status, 200, "/phpmyadmin redirects to /phpmyadmin/");
    let config = std::fs::read_to_string(env.layout.bin().join("phpmyadmin/5.2.3/config.inc.php"))
        .expect("config");
    assert!(
        config.contains(&format!("['port'] = '{}'", env.cfg.database.port)),
        "{config}"
    );

    env.cfg.php.version = "8.4".into();
    env.stack
        .restart(ServiceKind::PhpFcgi, &env.cfg)
        .await
        .expect("switch PHP");
    let (status, body) = get(&env.url("/phpmyadmin/")).await;
    assert_eq!(
        (status, body.as_str()),
        (200, "pma 6.0.0 on 8.4"),
        "{}",
        env.logs()
    );

    let failures = env.stack.stop_all().await;
    assert!(failures.is_empty(), "{failures:?}");
}

#[tokio::test]
async fn nginx_db_ui_follows_the_active_php() {
    db_ui_follows_the_active_php(WebServer::Nginx).await;
}

#[tokio::test]
async fn apache_db_ui_follows_the_active_php() {
    db_ui_follows_the_active_php(WebServer::Apache).await;
}

/// Both web servers and PHP 8.3 + 8.4, or `None` (the test is skipped).
fn profile_env() -> Option<Env> {
    let phps = std::env::var("LUMINA_TEST_PHP_DIRS").unwrap_or_default();
    if std::env::var("LUMINA_TEST_APACHE_DIR").is_err()
        || !(phps.contains("8.3") && phps.contains("8.4"))
    {
        eprintln!("needs nginx, Apache and PHP 8.3 + 8.4");
        return None;
    }
    let env = setup(WebServer::Nginx)?;
    let blog = env.layout.www().join("blog");
    std::fs::create_dir_all(&blog).expect("mkdir");
    std::fs::write(
        blog.join("index.php"),
        r#"<?php echo PHP_MAJOR_VERSION . "." . PHP_MINOR_VERSION . " " . ($_SERVER['SERVER_SOFTWARE'] ?? '');"#,
    )
    .expect("index");
    Some(env)
}

fn profile(name: &str, web: WebServer, php: &str) -> lumina_config::schema::Profile {
    lumina_config::schema::Profile {
        name: name.into(),
        web,
        php: php.into(),
        ..Default::default()
    }
}

/// Processes running from `bin/<rel>` (e.g. `nginx` or `php/8.3.35`).
fn running_from(env: &Env, rel: &str) -> usize {
    let root = env.layout.bin().join(rel);
    processes_under(&root).len()
}

fn php_dir(env: &Env, minor: &str) -> String {
    let dir = std::fs::read_dir(env.layout.bin().join("php"))
        .expect("php dir")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .find(|v| v.starts_with(&format!("{minor}.")))
        .expect("php version");
    format!("php/{dir}")
}

/// Blueprint §5: "Nginx + PHP 8.3" ↔ "Apache + PHP 8.4", 50 switches. After
/// each one the site answers from the right server and PHP, and nothing of
/// the previous stack is still running.
#[tokio::test]
async fn profile_switching_50_times_leaves_no_stale_processes() {
    let Some(mut env) = profile_env() else { return };
    let legacy = profile("Legacy", WebServer::Nginx, "8.3");
    let modern = profile("Modern", WebServer::Apache, "8.4");
    legacy.apply(&mut env.cfg);
    env.start_web().await.expect("start");
    let (php83, php84) = (php_dir(&env, "8.3"), php_dir(&env, "8.4"));

    let started = std::time::Instant::now();
    for round in 0..50 {
        let (next, server, php, old_web, old_php) = if round % 2 == 0 {
            (&modern, "Apache", "8.4", "nginx", &php83)
        } else {
            (&legacy, "nginx", "8.3", "apache", &php84)
        };
        let old = env.cfg.clone();
        next.apply(&mut env.cfg);
        let plan = env
            .stack
            .switch(&old, &env.cfg)
            .await
            .unwrap_or_else(|e| panic!("round {round}: {e}\n{}", env.logs()));
        assert_eq!(plan.stop.len(), 2, "round {round}: web + PHP change");

        let (status, body) = get_host(&env, "blog.test", "/").await;
        assert_eq!(status, 200, "round {round}");
        assert!(
            body.starts_with(php) && body.contains(server),
            "round {round}: {body}"
        );
        assert_eq!(
            running_from(&env, old_web),
            0,
            "round {round}: {old_web} left running"
        );
        assert_eq!(
            running_from(&env, old_php),
            0,
            "round {round}: {old_php} left running"
        );
    }
    println!("50 switches in {:?}", started.elapsed());

    let failures = env.stack.stop_all().await;
    assert!(failures.is_empty(), "{failures:?}");
    assert!(
        processes_under(&env.layout.bin()).is_empty(),
        "leftovers after stop_all"
    );
}

/// A profile that cannot start (here: a PHP version that is not installed)
/// rolls back to the previous stack, which keeps serving.
#[tokio::test]
async fn failing_profile_rolls_back() {
    let Some(mut env) = profile_env() else { return };
    profile("Current", WebServer::Nginx, "8.4").apply(&mut env.cfg);
    env.start_web().await.expect("start");

    let old = env.cfg.clone();
    let mut broken = env.cfg.clone();
    profile("Broken", WebServer::Apache, "7.0").apply(&mut broken);
    let err = env
        .stack
        .switch(&old, &broken)
        .await
        .expect_err("PHP 7.0 is not installed");
    let msg = err.to_string();
    assert!(
        msg.contains("7.0") && msg.contains("previous stack was restored"),
        "{msg}"
    );

    let (status, body) = get_host(&env, "blog.test", "/").await;
    assert_eq!(status, 200);
    assert!(body.starts_with("8.4") && body.contains("nginx"), "{body}");
    assert_eq!(running_from(&env, "apache"), 0);

    // A port conflict on the new web server's HTTPS port fails the same way.
    let holder = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind");
    let mut conflicting = env.cfg.clone();
    profile("Busy port", WebServer::Apache, "8.4").apply(&mut conflicting);
    conflicting.web.https_port = holder.local_addr().expect("addr").port();
    let err = env
        .stack
        .switch(&old, &conflicting)
        .await
        .expect_err("busy port");
    assert!(
        err.to_string().contains("previous stack was restored"),
        "{err}"
    );
    let (_, body) = get_host(&env, "blog.test", "/").await;
    assert!(body.contains("nginx"), "{body}");

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
