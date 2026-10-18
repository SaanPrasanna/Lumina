//! Environment and `php.ini` for a php-cgi pool.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use lumina_core::Result;
use lumina_core::constants::MIN_PHP_FCGI_CHILDREN;
use lumina_core::paths::Layout;

use super::versions::{PhpInstall, ext_dir};
use crate::template::{Templates, forward_slashes};

/// Extensions enabled in a generated php.ini, when the build ships them.
const EXTENSIONS: &[&str] = &[
    "bz2",
    "curl",
    "exif",
    "fileinfo",
    "gd",
    "gettext",
    "intl",
    "mbstring",
    "mysqli",
    "openssl",
    "pgsql",
    "pdo_mysql",
    "pdo_pgsql",
    "pdo_sqlite",
    "soap",
    "sockets",
    "sodium",
    "sqlite3",
    "xsl",
    "zip",
];

/// `PHP_FCGI_CHILDREN` actually used: never below the minimum.
pub fn effective_children(configured: u32) -> u32 {
    configured.max(MIN_PHP_FCGI_CHILDREN)
}

/// Environment for php-cgi, added to the inherited one.
pub fn fcgi_env(
    install: &PhpInstall,
    ini_dir: &Path,
    children: u32,
    max_requests: u32,
) -> Vec<(OsString, OsString)> {
    let mut env = vec![
        (
            "PHP_FCGI_CHILDREN".into(),
            effective_children(children).to_string().into(),
        ),
        (
            "PHP_FCGI_MAX_REQUESTS".into(),
            max_requests.to_string().into(),
        ),
        // Makes php-cgi load our php.ini, not one found elsewhere.
        ("PHPRC".into(), ini_dir.as_os_str().to_owned()),
    ];
    if cfg!(windows) {
        // Extensions such as intl load DLLs from the PHP folder; they are only
        // found if that folder is on PATH.
        let mut path = install.dir.as_os_str().to_owned();
        if let Some(existing) = std::env::var_os("PATH") {
            path.push(";");
            path.push(existing);
        }
        env.push(("PATH".into(), path));
    }
    env
}

/// php.ini settings that switch Xdebug on: step debugging when a request
/// carries the trigger (XDEBUG_TRIGGER cookie/parameter, browser extension),
/// to an IDE listening on 127.0.0.1:9003, plus the development helpers.
/// `zend_extension` says "xdebug"; [`resolve_xdebug`] turns it into a path.
pub fn xdebug_ini() -> Vec<(String, String)> {
    [
        ("zend_extension", "xdebug"),
        ("xdebug.mode", "debug,develop"),
        ("xdebug.start_with_request", "trigger"),
        ("xdebug.client_host", "127.0.0.1"),
        ("xdebug.client_port", "9003"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v.to_owned()))
    .collect()
}

/// The Xdebug extension for `install`: its own `ext/` folder first, then the
/// build Quick Add installed for this PHP version (`bin/xdebug/<x>-php8.4/`).
pub fn xdebug_extension(layout: &Layout, install: &PhpInstall) -> Option<PathBuf> {
    let own = ext_dir(install).join(if cfg!(windows) {
        "php_xdebug.dll"
    } else {
        "xdebug.so"
    });
    if own.is_file() {
        return Some(own);
    }
    let minor: String = install
        .version
        .split('.')
        .take(2)
        .collect::<Vec<_>>()
        .join(".");
    lumina_packages::installed::list(&layout.bin().join("xdebug"))
        .into_iter()
        .filter(|i| i.version.ends_with(&format!("-php{minor}")))
        .map(|i| i.dir.join("php_xdebug.dll"))
        .find(|p| p.is_file())
}

/// Points `zend_extension=xdebug` at the extension for `install`, or drops
/// every Xdebug setting when there is none (PHP would refuse to start).
/// Returns the settings and whether Xdebug is loaded.
pub fn resolve_xdebug(
    layout: &Layout,
    install: &PhpInstall,
    ini: &[(String, String)],
) -> (Vec<(String, String)>, bool) {
    let wanted = ini
        .iter()
        .any(|(k, v)| k == "zend_extension" && v == "xdebug");
    if !wanted {
        return (ini.to_vec(), false);
    }
    match xdebug_extension(layout, install) {
        Some(path) => {
            let path = forward_slashes(&path);
            let out = ini
                .iter()
                .map(|(k, v)| match (k.as_str(), v.as_str()) {
                    ("zend_extension", "xdebug") => (k.clone(), path.clone()),
                    _ => (k.clone(), v.clone()),
                })
                .collect();
            (out, true)
        }
        None => {
            tracing::warn!(
                "Xdebug is on, but PHP {} has no Xdebug extension; install it from Quick Add (Windows) or with pecl",
                install.version
            );
            let out = ini
                .iter()
                .filter(|(k, v)| {
                    !(k.starts_with("xdebug.") || (k == "zend_extension" && v == "xdebug"))
                })
                .cloned()
                .collect();
            (out, false)
        }
    }
}

/// `etc/php/<version>/`, where this version's php.ini lives.
pub fn ini_dir(layout: &Layout, install: &PhpInstall) -> PathBuf {
    layout.etc().join("php").join(&install.version)
}

/// Writes `etc/php/<version>/php.ini` from the template if it does not exist yet.
/// An existing file is never touched, so the user's edits survive.
pub fn ensure_ini(layout: &Layout, templates: &Templates, install: &PhpInstall) -> Result<PathBuf> {
    let dir = ini_dir(layout, install);
    let ini = dir.join("php.ini");
    if ini.exists() {
        return Ok(ini);
    }

    let ext_dir = ext_dir(install);
    let available = |name: &str| {
        let file = if cfg!(windows) {
            format!("php_{name}.dll")
        } else {
            format!("{name}.so")
        };
        ext_dir.join(file).is_file()
    };
    let extensions: Vec<&str> = EXTENSIONS
        .iter()
        .copied()
        .filter(|e| available(e))
        .collect();

    let mut ctx = tera::Context::new();
    ctx.insert("version", &install.version);
    ctx.insert("windows", &cfg!(windows));
    ctx.insert("extension_dir", &forward_slashes(&ext_dir));
    ctx.insert("extensions", &extensions);
    // PHP 8.5+ has opcache built in; older builds load it as a zend_extension.
    ctx.insert("opcache_extension", &available("opcache"));
    ctx.insert("tmp_dir", &forward_slashes(&layout.tmp()));
    ctx.insert(
        "mysql_socket",
        &crate::database::Database::socket(layout).map(|s| forward_slashes(&s)),
    );
    ctx.insert(
        "error_log",
        &forward_slashes(&layout.logs().join("php_errors.log")),
    );

    let text = templates.render("php/php.ini.tera", &ctx)?;
    lumina_core::fs::write_atomic(&ini, text.as_bytes())?;
    tracing::info!("wrote {}", ini.display());
    Ok(ini)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn templates() -> Templates {
        Templates::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/templates"))
    }

    #[test]
    fn xdebug_is_pointed_at_its_extension_or_left_out() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let layout = Layout::new(tmp.path().join("root"));
        let php = tmp.path().join("php");
        std::fs::create_dir_all(php.join("ext")).expect("ext");
        let install = PhpInstall {
            version: "8.4.26".into(),
            dir: php.clone(),
            exe: php.join("php-cgi"),
            homebrew: None,
        };
        let mut ini = vec![("SMTP".to_owned(), "127.0.0.1".to_owned())];
        ini.extend(xdebug_ini());

        let (out, loaded) = resolve_xdebug(&layout, &install, &ini);
        assert!(!loaded);
        assert_eq!(
            out,
            [("SMTP".to_owned(), "127.0.0.1".to_owned())],
            "no extension: nothing of Xdebug"
        );

        let dll = layout.bin().join("xdebug").join("3.5.3-php8.4");
        std::fs::create_dir_all(&dll).expect("xdebug");
        std::fs::write(dll.join("php_xdebug.dll"), b"").expect("dll");
        if cfg!(windows) {
            let (out, loaded) = resolve_xdebug(&layout, &install, &ini);
            assert!(loaded);
            let ext = out
                .iter()
                .find(|(k, _)| k == "zend_extension")
                .expect("zend_extension");
            assert!(ext.1.ends_with("3.5.3-php8.4/php_xdebug.dll"), "{}", ext.1);
            assert!(
                out.iter()
                    .any(|(k, v)| k == "xdebug.mode" && v == "debug,develop")
            );
        }
    }

    #[test]
    fn children_never_below_minimum() {
        assert_eq!(effective_children(2), 4);
        assert_eq!(effective_children(20), 20);
    }

    #[test]
    fn env_sets_fcgi_variables() {
        let install = PhpInstall {
            version: "8.4.26".into(),
            dir: PathBuf::from("php"),
            exe: PathBuf::from("php/php-cgi"),
            homebrew: None,
        };
        let env = fcgi_env(&install, Path::new("ini"), 1, 500);
        let get = |k: &str| env.iter().find(|(key, _)| key == k).map(|(_, v)| v.clone());
        assert_eq!(get("PHP_FCGI_CHILDREN"), Some("4".into()));
        assert_eq!(get("PHP_FCGI_MAX_REQUESTS"), Some("500".into()));
        assert_eq!(get("PHPRC"), Some("ini".into()));
    }

    #[test]
    fn renders_ini_once_with_available_extensions_only() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let layout = Layout::new(tmp.path().join("root"));
        let php = tmp.path().join("php");
        std::fs::create_dir_all(php.join("ext")).expect("ext dir");
        let ext = |n: &str| {
            if cfg!(windows) {
                format!("php_{n}.dll")
            } else {
                format!("{n}.so")
            }
        };
        for name in ["mbstring", "opcache"] {
            std::fs::write(php.join("ext").join(ext(name)), b"").expect("ext");
        }
        let install = PhpInstall {
            version: "8.4.26".into(),
            dir: php.clone(),
            exe: php.join("php-cgi"),
            homebrew: None,
        };

        let ini = ensure_ini(&layout, &templates(), &install).expect("render");
        let text = std::fs::read_to_string(&ini).expect("read");

        assert!(text.contains("extension=mbstring"), "{text}");
        assert!(!text.contains("extension=curl"), "curl is not installed");
        assert!(text.contains("zend_extension=opcache"));
        assert!(!text.contains('\\'), "paths use forward slashes");

        // User edits survive: a second call leaves the file alone.
        std::fs::write(&ini, "; mine").expect("edit");
        ensure_ini(&layout, &templates(), &install).expect("again");
        assert_eq!(std::fs::read_to_string(&ini).expect("read"), "; mine");
    }
}
