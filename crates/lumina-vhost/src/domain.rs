//! Folder name -> domain (my-app -> my-app.test) and document root detection (public/, web/).

use std::path::{Path, PathBuf};

use lumina_platform::hosts;

/// Document-root candidates, in order: Laravel, Symfony/Craft, classic hosting.
/// The project folder itself is the fallback.
pub const DOCROOT_CANDIDATES: [&str; 3] = ["public", "web", "public_html"];

/// The host label for a folder name: lowercase ASCII letters and digits, every
/// other run of characters becomes one `-`, dots separate labels. `None` if
/// nothing usable is left (e.g. a name made only of symbols).
///
/// `My App` -> `my-app`, `shop_v2` -> `shop-v2`, `example.com` -> `example.com`.
pub fn slugify(name: &str) -> Option<String> {
    let labels: Vec<String> = name
        .split('.')
        .map(|part| {
            let mut out = String::new();
            let mut dash = false;
            for c in part.chars() {
                if c.is_ascii_alphanumeric() {
                    if dash && !out.is_empty() {
                        out.push('-');
                    }
                    dash = false;
                    out.push(c.to_ascii_lowercase());
                } else {
                    dash = true;
                }
            }
            out.truncate(63);
            out.trim_end_matches('-').to_owned()
        })
        .filter(|label| !label.is_empty())
        .collect();
    let slug = labels.join(".");
    (!slug.is_empty()).then_some(slug)
}

/// `blog` + `test` -> `blog.test`, if the result is a valid host name.
pub fn domain_for(folder: &str, tld: &str) -> Option<String> {
    let domain = format!("{}.{tld}", slugify(folder)?);
    hosts::is_valid_host(&domain).then_some(domain)
}

/// A project folder as seen on disk: what the watcher compares and what
/// docroot detection needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Folder {
    pub name: String,
    pub path: PathBuf,
    /// Which of [`DOCROOT_CANDIDATES`] exist as directories.
    pub candidates: [bool; 3],
}

impl Folder {
    pub fn read(path: PathBuf) -> Option<Self> {
        let name = path.file_name()?.to_str()?.to_owned();
        let candidates = DOCROOT_CANDIDATES.map(|c| path.join(c).is_dir());
        Some(Self {
            name,
            path,
            candidates,
        })
    }

    /// The first existing candidate, else the folder itself.
    pub fn detected_docroot(&self) -> (PathBuf, Option<&'static str>) {
        DOCROOT_CANDIDATES
            .iter()
            .zip(self.candidates)
            .find(|(_, exists)| *exists)
            .map_or((self.path.clone(), None), |(sub, _)| {
                (self.path.join(sub), Some(*sub))
            })
    }
}

/// The project folders directly inside `www`, sorted by name. Dot-folders,
/// files and names that are not valid UTF-8 are skipped. A missing `www`
/// yields an empty list.
pub fn list_folders(www: &Path) -> Vec<Folder> {
    let Ok(entries) = std::fs::read_dir(www) else {
        return Vec::new();
    };
    let mut folders: Vec<Folder> = entries
        .filter_map(|e| e.ok())
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        // `Path::is_dir` follows links, so linked projects count too.
        .filter(|e| e.path().is_dir())
        .filter_map(|e| Folder::read(e.path()))
        .collect();
    folders.sort_by(|a, b| a.name.cmp(&b.name));
    folders
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs() {
        for (name, slug) in [
            ("blog", Some("blog")),
            ("My App", Some("my-app")),
            ("shop_v2", Some("shop-v2")),
            ("--x--", Some("x")),
            ("a  &  b", Some("a-b")),
            ("example.com", Some("example.com")),
            ("..a..b..", Some("a.b")),
            ("Café", Some("caf")),
            ("日本", None),
            ("!!!", None),
            ("", None),
        ] {
            assert_eq!(slugify(name).as_deref(), slug, "{name:?}");
        }
        assert_eq!(slugify(&"a".repeat(80)).map(|s| s.len()), Some(63));
    }

    #[test]
    fn domains() {
        assert_eq!(domain_for("My App", "test").as_deref(), Some("my-app.test"));
        assert_eq!(domain_for("!!!", "test"), None);
    }

    #[test]
    fn lists_folders_and_detects_docroots_in_order() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let www = tmp.path();
        for dir in [
            "laravel/public",
            "laravel/web",
            "symfony/web",
            "classic/public_html",
            "plain",
            ".git",
        ] {
            std::fs::create_dir_all(www.join(dir)).expect("mkdir");
        }
        std::fs::write(www.join("index.php"), "<?php").expect("file");
        std::fs::write(www.join("plain/public"), "a file, not a dir").expect("file");

        let folders = list_folders(www);
        let names: Vec<&str> = folders.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["classic", "laravel", "plain", "symfony"]);
        let docroots: Vec<Option<&str>> = folders.iter().map(|f| f.detected_docroot().1).collect();
        assert_eq!(
            docroots,
            [Some("public_html"), Some("public"), None, Some("web")]
        );
        assert_eq!(folders[2].detected_docroot().0, www.join("plain"));

        assert!(list_folders(&www.join("missing")).is_empty());
    }
}
