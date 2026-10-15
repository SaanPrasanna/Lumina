//! Types and helpers shared by the web servers (nginx, Apache).

use std::path::{Path, PathBuf};

use lumina_config::schema::WebServer;
use lumina_core::types::ServiceStatus;
use lumina_core::{Error, Result};
use lumina_process::ports;
use lumina_ssl::Tls;
use lumina_vhost::{Site, render};
use serde::Serialize;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

use crate::template::{ChangeSet, Templates, forward_slashes};

/// What a web server config is rendered from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebContext {
    pub http_port: u16,
    /// Where PHP FastCGI listens (the actual port, which may be the fallback).
    pub php_port: u16,
    /// Served at http://localhost.
    pub www_root: PathBuf,
    /// Automatic virtual hosts, one `auto.<domain>.conf` each.
    pub sites: Vec<Site>,
    /// HTTPS, when `general.auto_ssl` is on.
    pub tls: Option<Tls>,
    /// phpMyAdmin at /phpmyadmin/ on localhost, if one is installed that runs
    /// on the active PHP.
    pub db_ui: Option<DbUi>,
}

/// The phpMyAdmin install served at http://localhost/phpmyadmin/.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DbUi {
    pub version: String,
    pub dir: PathBuf,
    /// The PHP version it was chosen for.
    pub php_version: String,
    #[serde(skip)]
    pub secret: String,
    #[serde(skip)]
    pub db_port: u16,
    #[serde(skip)]
    pub tmp_dir: PathBuf,
}

/// Template values for the localhost site (`db_ui*`) and phpMyAdmin's own
/// `config.inc.php`, written through `changes`.
pub fn render_db_ui(
    templates: &Templates,
    changes: &mut ChangeSet,
    c: &mut tera::Context,
    ctx: &WebContext,
) -> Result<()> {
    c.insert("db_ui", &ctx.db_ui.is_some());
    let Some(ui) = &ctx.db_ui else {
        return Ok(());
    };
    c.insert("db_ui_version", &ui.version);
    c.insert("db_ui_dir", &forward_slashes(&ui.dir));
    c.insert("db_ui_php", &ui.php_version);
    std::fs::create_dir_all(&ui.tmp_dir).map_err(|source| Error::Io {
        path: ui.tmp_dir.clone(),
        source,
    })?;
    let mut pc = tera::Context::new();
    pc.insert("version", &ui.version);
    pc.insert("secret", &ui.secret);
    pc.insert("port", &ui.db_port);
    pc.insert("tmp_dir", &forward_slashes(&ui.tmp_dir));
    templates.render_into(
        changes,
        "phpmyadmin/config.inc.php.tera",
        &pc,
        &ui.dir.join("config.inc.php"),
    )?;
    Ok(())
}

impl WebContext {
    pub fn https_port(&self) -> Option<u16> {
        self.tls.as_ref().map(|t| t.https_port)
    }
}

/// Template values shared by every config file: `ssl`, `https_port`,
/// `localhost_cert`/`_key`/`_serial`.
pub fn insert_tls(c: &mut tera::Context, ctx: &WebContext) {
    c.insert("ssl", &ctx.tls.is_some());
    if let Some(tls) = &ctx.tls {
        c.insert("https_port", &tls.https_port);
        c.insert("localhost_cert", &forward_slashes(&tls.localhost.cert));
        c.insert("localhost_key", &forward_slashes(&tls.localhost.key));
        c.insert("localhost_serial", &tls.localhost.serial);
    }
}

/// Fails with the owner's name if the HTTP port, or the HTTPS port when TLS is
/// on, is taken. Ports `running` already listens on are skipped.
pub fn check_ports(ctx: &WebContext, running: Option<&WebRuntime>) -> Result<()> {
    if running.is_none_or(|r| r.http_port != ctx.http_port) {
        check_port(ctx.http_port)?;
    }
    if let Some(https) = ctx.https_port()
        && running.is_none_or(|r| r.https_port != Some(https))
    {
        check_port(https)?;
    }
    Ok(())
}

/// Renders `auto.<domain>.conf` from `template` into `sites_dir` for every
/// site (skipping domains the user has their own `<domain>.conf` for) and
/// deletes `auto.*.conf` files of sites that are gone, all through `changes`
/// so a config that fails the server's test is undone as a whole.
pub fn render_sites(
    templates: &Templates,
    changes: &mut ChangeSet,
    template: &str,
    sites_dir: &Path,
    ctx: &WebContext,
) -> Result<()> {
    let mut desired = Vec::new();
    for site in &ctx.sites {
        if !render::needs_auto_file(sites_dir, site) {
            continue;
        }
        let file = render::auto_file_name(&site.domain);
        let mut c = tera::Context::new();
        c.insert("name", &site.name);
        c.insert("domain", &site.domain);
        c.insert("root", &forward_slashes(&site.docroot));
        c.insert("http_port", &ctx.http_port);
        c.insert("user_file", &render::user_file_name(&site.domain));
        c.insert("auto_file", &file);
        let leaf = ctx.tls.as_ref().and_then(|t| t.sites.get(&site.domain));
        c.insert("ssl", &leaf.is_some());
        if let (Some(tls), Some(leaf)) = (&ctx.tls, leaf) {
            c.insert("https_port", &tls.https_port);
            c.insert("force_https", &tls.force_https);
            let suffix = match tls.https_port {
                443 => String::new(),
                port => format!(":{port}"),
            };
            c.insert("https_suffix", &suffix);
            c.insert("cert", &forward_slashes(&leaf.cert));
            c.insert("key", &forward_slashes(&leaf.key));
            c.insert("serial", &leaf.serial);
            c.insert("cert_expires", &date(leaf.not_after));
        }
        templates.render_into(changes, template, &c, &sites_dir.join(&file))?;
        desired.push(file);
    }
    for stale in render::stale_auto_files(sites_dir, &desired) {
        changes.remove(&stale)?;
    }
    Ok(())
}

/// What is running right now.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WebRuntime {
    pub server: WebServer,
    pub version: String,
    pub http_port: u16,
    /// Set while HTTPS is served.
    pub https_port: Option<u16>,
    pub php_port: u16,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WebStatus {
    pub server: WebServer,
    pub status: ServiceStatus,
    pub runtime: Option<WebRuntime>,
    pub pid: Option<u32>,
    /// Live server processes (master + workers).
    pub processes: usize,
    pub error: Option<String>,
    /// The server's error log.
    pub log_file: PathBuf,
}

/// Result of a reload request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ReloadOutcome {
    /// Nothing changed on disk, so no signal was sent.
    Unchanged,
    /// The running server re-read its config; `signals` reload commands were
    /// sent (1 normally, 2 if the first was not confirmed).
    Reloaded { signals: u32 },
    /// The server was stopped and started (Apache on Windows has no graceful
    /// reload outside service mode).
    Restarted,
}

/// `2027-11-15` from Unix seconds.
fn date(unix: i64) -> String {
    let days = unix.div_euclid(86_400);
    // Civil-from-days (Howard Hinnant), proleptic Gregorian.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Fails with the owner's name if something already listens on `port`.
pub fn check_port(port: u16) -> Result<()> {
    if ports::is_free(port) {
        return Ok(());
    }
    Err(Error::PortInUse {
        port,
        owner: ports::owner(port),
    })
}

/// Puts Lumina's welcome page into `www_root` if it has no index file yet, so
/// http://localhost shows something on a fresh install. Never overwrites.
pub fn ensure_landing_page(www_root: &Path, bundled_page: &Path) -> Result<()> {
    std::fs::create_dir_all(www_root).map_err(|source| Error::Io {
        path: www_root.to_path_buf(),
        source,
    })?;
    let has_index = ["index.php", "index.html", "index.htm"]
        .iter()
        .any(|name| www_root.join(name).exists());
    if has_index {
        return Ok(());
    }
    let target = www_root.join("index.html");
    std::fs::copy(bundled_page, &target).map_err(|source| Error::Io {
        path: target,
        source,
    })?;
    Ok(())
}

/// The PIDs among `pids` whose executable name is `stem` (with or without
/// `.exe`). Process groups also contain helpers such as `conhost.exe` on
/// Windows, which must not be counted as server processes. The executable's
/// file name is preferred over the process name, which Linux truncates to 15
/// characters.
pub fn named(pids: &[u32], stem: &str) -> Vec<u32> {
    if pids.is_empty() {
        return Vec::new();
    }
    let sys_pids: Vec<Pid> = pids.iter().map(|&p| Pid::from_u32(p)).collect();
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::Some(&sys_pids),
        true,
        ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet),
    );
    let with_suffix = format!("{stem}{}", std::env::consts::EXE_SUFFIX);
    let matches =
        |name: &str| name.eq_ignore_ascii_case(stem) || name.eq_ignore_ascii_case(&with_suffix);
    sys_pids
        .iter()
        .filter(|pid| {
            sys.process(**pid)
                .is_some_and(|p| match p.exe().and_then(|e| e.file_name()) {
                    Some(file) => matches(&file.to_string_lossy()),
                    None => matches(&p.name().to_string_lossy()),
                })
        })
        .map(|pid| pid.as_u32())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert_eq!(date(0), "1970-01-01");
        assert_eq!(date(1_791_590_400), "2026-10-10");
        assert_eq!(date(951_782_400), "2000-02-29");
    }

    #[test]
    fn landing_page_is_copied_once_and_never_overwrites() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let page = tmp.path().join("bundled.html");
        std::fs::write(&page, "welcome").expect("write");
        let www = tmp.path().join("www");

        ensure_landing_page(&www, &page).expect("first");
        assert_eq!(
            std::fs::read_to_string(www.join("index.html")).expect("read"),
            "welcome"
        );

        std::fs::write(www.join("index.html"), "mine").expect("edit");
        ensure_landing_page(&www, &page).expect("second");
        assert_eq!(
            std::fs::read_to_string(www.join("index.html")).expect("read"),
            "mine"
        );
    }

    #[test]
    fn existing_php_index_means_no_landing_page() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let www = tmp.path().join("www");
        std::fs::create_dir_all(&www).expect("mkdir");
        std::fs::write(www.join("index.php"), "<?php").expect("write");
        ensure_landing_page(&www, &tmp.path().join("missing.html")).expect("noop");
        assert!(!www.join("index.html").exists());
    }

    #[test]
    fn busy_port_names_its_owner() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        match check_port(port) {
            Err(Error::PortInUse { port: p, owner }) => {
                assert_eq!(p, port);
                assert!(owner.is_some_and(|o| o.contains(&std::process::id().to_string())));
            }
            other => panic!("expected PortInUse, got {other:?}"),
        }
    }

    #[test]
    fn named_filters_by_executable() {
        let me = std::process::id();
        let exe = std::env::current_exe().expect("exe");
        let stem = exe
            .file_stem()
            .expect("stem")
            .to_string_lossy()
            .into_owned();
        assert_eq!(named(&[me], &stem), vec![me]);
        assert!(named(&[me], "definitely-not-this").is_empty());
    }
}
