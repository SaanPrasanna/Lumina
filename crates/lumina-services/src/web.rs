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
    /// php-cgi pools of other PHP versions, for sites that ask for one.
    pub php_pools: Vec<PhpPool>,
    /// Site name → the pool it uses; sites not listed use `php_port`.
    pub site_php: std::collections::BTreeMap<String, PhpPool>,
    /// Adminer at /adminer/ on localhost: the newest installed one's folder.
    pub adminer: Option<PathBuf>,
    /// Listen on every interface (LAN sharing), not only 127.0.0.1.
    pub lan: bool,
}

/// A php-cgi pool for sites with their own PHP version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PhpPool {
    /// Usable in config names: `8_3_12`.
    pub key: String,
    pub version: String,
    pub port: u16,
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
    c.insert(
        "adminer_dir",
        &ctx.adminer
            .as_deref()
            .map(forward_slashes)
            .unwrap_or_default(),
    );
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
/// `localhost_cert`/`_key`/`_serial`, and where to listen: `bind`
/// (`127.0.0.1`, or `0.0.0.0` with LAN sharing) and `vhost_ip` (Apache's
/// `<VirtualHost>` address: `127.0.0.1` or `*`).
pub fn insert_tls(c: &mut tera::Context, ctx: &WebContext) {
    c.insert("lan", &ctx.lan);
    c.insert("bind", if ctx.lan { "0.0.0.0" } else { "127.0.0.1" });
    c.insert("vhost_ip", if ctx.lan { "*" } else { "127.0.0.1" });
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
        // `blog.test` → `blog`: also answered as blog.<lan-ip>.nip.io.
        let label = site
            .domain
            .rsplit_once('.')
            .map_or(site.domain.as_str(), |(l, _)| l);
        c.insert("lan", &ctx.lan);
        c.insert("bind", if ctx.lan { "0.0.0.0" } else { "127.0.0.1" });
        c.insert("vhost_ip", if ctx.lan { "*" } else { "127.0.0.1" });
        c.insert("lan_label", label);
        c.insert("lan_label_re", &label.replace('.', "\\."));
        c.insert("app", &site.app.is_some());
        let pool = ctx.site_php.get(&site.name);
        c.insert("php_pool", &pool.map_or("", |p| p.key.as_str()));
        c.insert("php_pool_port", &pool.map_or(0, |p| p.port));
        if let Some(app) = &site.app {
            c.insert("app_port", &app.port);
            c.insert("app_framework", &app.framework);
            c.insert("app_command_html", &html_for_config(&app.command));
        }
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

/// `text` for an HTML page inside a double-quoted nginx or Apache string:
/// HTML-escaped, so no quote, `$`, `%` or backslash of a command reaches the
/// config (inside a quoted string nothing else is special to either server).
fn html_for_config(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            '$' => out.push_str("&#36;"),
            '\\' => out.push_str("&#92;"),
            '%' => out.push_str("&#37;"),
            c if c.is_control() => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod html_tests {
    #[test]
    fn commands_cannot_break_out_of_the_config_string() {
        let html = super::html_for_config(r#"a "b" $x\y %z <i> & 'q'"#);
        assert!(!html.contains(['"', '$', '\\', '%', '<', '\'']), "{html}");
        assert_eq!(super::html_for_config("npm run dev"), "npm run dev");
    }
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

const SHIPPED_LANDING_PAGES: [&str; 5] = [
    "3599fdd03ebd57c4565406205dd43a468f52bb39e5976b9221f15e8c865786a9",
    "55d5faf3c5a0fa6a003d6833152a2d1358ec44f34e278ccde75bc3c6825ca0ab",
    "6e70f1f03937e7358b486523b29fbb4a7e258fd455d464a9bf3314b6c4262c45",
    "cb804e7fb1699b5a8410a268dd6027ebd0e00106b4a402f9f4f83d4d0624dcc9",
    "5f8cd5ff029e0088a97b45ef8049bda33406d3df30b53dfafdb9cc744c373e3a",
];

fn page_hash(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let text = String::from_utf8_lossy(bytes).replace("\r\n", "\n");
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub fn ensure_landing_page(www_root: &Path, bundled_page: &Path) -> Result<()> {
    std::fs::create_dir_all(www_root).map_err(|source| Error::Io {
        path: www_root.to_path_buf(),
        source,
    })?;
    let target = www_root.join("index.html");
    if ["index.php", "index.htm"]
        .iter()
        .any(|name| www_root.join(name).exists())
    {
        return Ok(());
    }
    if let Ok(current) = std::fs::read(&target) {
        let current = page_hash(&current);
        let bundled = std::fs::read(bundled_page).map(|b| page_hash(&b));
        if !SHIPPED_LANDING_PAGES.contains(&current.as_str()) || bundled.is_ok_and(|b| b == current)
        {
            return Ok(());
        }
        tracing::info!("updating the welcome page {}", target.display());
    }
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
    fn an_untouched_older_landing_page_is_replaced() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let www = tmp.path().join("www");
        std::fs::create_dir_all(&www).expect("mkdir");
        let page = tmp.path().join("bundled.html");
        std::fs::write(&page, "new welcome").expect("write");
        // A page someone wrote: not on the list, so it is kept.
        let old = "old welcome\r\n";
        std::fs::write(www.join("index.html"), old).expect("old");

        assert!(!SHIPPED_LANDING_PAGES.contains(&page_hash(old.as_bytes()).as_str()));
        ensure_landing_page(&www, &page).expect("kept");
        assert_eq!(
            std::fs::read_to_string(www.join("index.html")).expect("read"),
            old
        );

        // An unedited copy of what Lumina shipped is replaced.
        let shipped = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/www/index.html"),
        )
        .expect("bundled page");
        std::fs::write(www.join("index.html"), &shipped).expect("shipped copy");
        ensure_landing_page(&www, &page).expect("replace");
        assert_eq!(
            std::fs::read_to_string(www.join("index.html")).expect("read"),
            "new welcome"
        );
    }

    #[test]
    fn the_current_landing_page_is_listed() {
        let page = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/www/index.html");
        let hash = page_hash(&std::fs::read(page).expect("bundled page"));
        assert!(
            SHIPPED_LANDING_PAGES.contains(&hash.as_str()),
            "add {hash} to SHIPPED_LANDING_PAGES"
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
