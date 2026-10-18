//! Which vhost files Lumina writes into a web server's `sites/` folder.
//!
//! - `auto.<domain>.conf` is generated for each site and deleted when the site
//!   goes away. Lumina owns these files.
//! - `<domain>.conf` (no `auto.` prefix) is the user's own override: when it
//!   exists, Lumina generates nothing for that domain and never touches it.
//! - `00-localhost.conf` is the http://localhost site.
//!
//! The web-server drivers render the files from their templates; this module
//! decides names and what is stale.

use std::path::{Path, PathBuf};

use crate::sites::Site;

pub const AUTO_PREFIX: &str = "auto.";

/// `auto.blog.test.conf`.
pub fn auto_file_name(domain: &str) -> String {
    format!("{AUTO_PREFIX}{domain}.conf")
}

/// `blog.test.conf`, the user's override file name.
pub fn user_file_name(domain: &str) -> String {
    format!("{domain}.conf")
}

/// Whether `site` gets an `auto.` file: not if the user has their own `<domain>.conf`.
pub fn needs_auto_file(sites_dir: &Path, site: &Site) -> bool {
    !sites_dir.join(user_file_name(&site.domain)).exists()
}

/// `auto.*.conf` files in `sites_dir` that no desired site owns.
pub fn stale_auto_files(sites_dir: &Path, desired: &[String]) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(sites_dir) else {
        return Vec::new();
    };
    let mut stale: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            name.starts_with(AUTO_PREFIX)
                && name.ends_with(".conf")
                && !desired.contains(&name)
                && e.path().is_file()
        })
        .map(|e| e.path())
        .collect();
    stale.sort();
    stale
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sites::DocrootSource;

    #[test]
    fn user_files_win_and_stale_auto_files_are_found() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path();
        for f in [
            "00-localhost.conf",
            "auto.old.test.conf",
            "auto.blog.test.conf",
            "shop.test.conf",
            "auto.notes.txt",
        ] {
            std::fs::write(dir.join(f), "").expect("write");
        }
        let site = |domain: &str| Site {
            name: domain.into(),
            domain: domain.into(),
            path: dir.into(),
            docroot: dir.into(),
            docroot_source: DocrootSource::Detected(None),
            docroot_exists: true,
            app: None,
            php: None,
            node: false,
        };
        assert!(needs_auto_file(dir, &site("blog.test")));
        assert!(!needs_auto_file(dir, &site("shop.test")));

        let stale = stale_auto_files(dir, &[auto_file_name("blog.test")]);
        assert_eq!(stale, [dir.join("auto.old.test.conf")]);
    }
}
