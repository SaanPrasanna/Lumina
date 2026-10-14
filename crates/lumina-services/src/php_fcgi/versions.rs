//! Discovers installed PHP versions under `<data-root>/bin/php/<version>/`.

use std::path::{Path, PathBuf};

use crate::install::{self, Install};

/// A PHP install; `exe` is php-cgi.
pub type PhpInstall = Install;

/// `php-cgi.exe` on Windows, `php-cgi` elsewhere.
pub fn cgi_file_name() -> String {
    format!("php-cgi{}", std::env::consts::EXE_SUFFIX)
}

/// The extension folder of an install.
pub fn ext_dir(install: &PhpInstall) -> PathBuf {
    install.dir.join("ext")
}

/// All usable PHP installs, newest first. Windows zips put php-cgi at the root;
/// Unix builds use `bin/`.
pub async fn discover(php_root: &Path) -> Vec<PhpInstall> {
    install::discover(
        php_root,
        &["php-cgi", "bin/php-cgi"],
        &["-v"],
        parse_version,
    )
    .await
}

pub use install::select;

/// "PHP 8.4.26 (cgi-fcgi) ..." → "8.4.26".
fn parse_version(text: &str) -> Option<String> {
    let rest = text.trim_start().strip_prefix("PHP ")?;
    let version = rest.split_whitespace().next()?;
    version
        .starts_with(|c: char| c.is_ascii_digit())
        .then(|| version.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_php_cgi_banner() {
        let out = "PHP 8.4.26 (cgi-fcgi) (built: Sep 22 2026 15:11:32) (NTS Visual C++ 2022 x64)\n";
        assert_eq!(parse_version(out).as_deref(), Some("8.4.26"));
        assert_eq!(parse_version("garbage"), None);
    }
}
