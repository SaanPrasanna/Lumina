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
    "pdo_mysql",
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
