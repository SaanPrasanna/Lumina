//! The desired set of sites: every project folder that becomes `<slug>.<tld>`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use lumina_config::AppConfig;
use serde::Serialize;

use crate::domain::{self, Folder};

/// One automatic virtual host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Site {
    /// Folder name inside the projects folder (the key of `[sites.<name>]`).
    pub name: String,
    /// `blog.test`.
    pub domain: String,
    /// The project folder.
    pub path: PathBuf,
    /// What the web server serves.
    pub docroot: PathBuf,
    /// How the docroot was chosen.
    pub docroot_source: DocrootSource,
    /// `false` when an overridden docroot does not exist (the site still gets
    /// a vhost, which answers 404 until the folder is created).
    pub docroot_exists: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "subdir")]
pub enum DocrootSource {
    /// Detected: `public`, `web`, `public_html`, or `None` for the folder itself.
    Detected(Option<String>),
    /// Set in `[sites.<name>] docroot`.
    Configured,
}

/// A folder that does not become a site, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Skipped {
    pub name: String,
    pub reason: SkipReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum SkipReason {
    /// Nothing usable for a host name is left after slugifying.
    InvalidName,
    /// Another folder (listed first) already maps to this domain.
    DuplicateDomain { domain: String, taken_by: String },
    /// `[sites.<name>] enabled = false`.
    Disabled,
    /// The folder or docroot name has a character web-server configs cannot
    /// quote (`"`, `$`, `;`, braces, backslash, line breaks).
    UnsafePath,
}

/// Characters that would break or change the meaning of a quoted path in an
/// nginx or Apache config.
fn is_config_safe(name: &str) -> bool {
    !name.contains(['"', '$', ';', '{', '}', '\\', '\n', '\r'])
}

/// Every project folder, sorted by name, as sites or as skipped folders.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Scan {
    pub sites: Vec<Site>,
    pub skipped: Vec<Skipped>,
}

/// Plans sites for `folders` with the config's TLD and per-site overrides.
/// With `general.auto_vhosts` off there are no sites at all.
pub fn plan(folders: &[Folder], cfg: &AppConfig) -> Scan {
    let mut scan = Scan::default();
    if !cfg.general.auto_vhosts {
        return scan;
    }
    let mut taken: BTreeMap<String, String> = BTreeMap::new();
    for folder in folders {
        let overrides = cfg.sites.get(&folder.name);
        if overrides.is_some_and(|o| !o.enabled) {
            scan.skipped.push(Skipped {
                name: folder.name.clone(),
                reason: SkipReason::Disabled,
            });
            continue;
        }
        let configured = overrides
            .map(|o| o.docroot.trim())
            .filter(|d| !d.is_empty());
        let unsafe_override = configured.is_some_and(|d| {
            // Absolute Windows paths use backslashes legitimately.
            !is_config_safe(&d.replace('\\', "/"))
        });
        if !is_config_safe(&folder.name) || unsafe_override {
            scan.skipped.push(Skipped {
                name: folder.name.clone(),
                reason: SkipReason::UnsafePath,
            });
            continue;
        }
        let Some(domain) = domain::domain_for(&folder.name, &cfg.general.tld) else {
            scan.skipped.push(Skipped {
                name: folder.name.clone(),
                reason: SkipReason::InvalidName,
            });
            continue;
        };
        if let Some(owner) = taken.get(&domain) {
            scan.skipped.push(Skipped {
                name: folder.name.clone(),
                reason: SkipReason::DuplicateDomain {
                    domain,
                    taken_by: owner.clone(),
                },
            });
            continue;
        }
        taken.insert(domain.clone(), folder.name.clone());

        let (docroot, docroot_source) = match configured {
            Some(dir) => (folder.path.join(Path::new(dir)), DocrootSource::Configured),
            None => {
                let (dir, sub) = folder.detected_docroot();
                (dir, DocrootSource::Detected(sub.map(str::to_owned)))
            }
        };
        scan.sites.push(Site {
            name: folder.name.clone(),
            domain,
            path: folder.path.clone(),
            docroot_exists: docroot.is_dir(),
            docroot,
            docroot_source,
        });
    }
    scan
}

/// [`plan`] for the folders currently in `www`.
pub fn scan(www: &Path, cfg: &AppConfig) -> Scan {
    plan(&domain::list_folders(www), cfg)
}

#[cfg(test)]
mod tests {
    use lumina_config::schema::SiteOverride;

    use super::*;

    fn setup(dirs: &[&str]) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().expect("tempdir");
        for d in dirs {
            std::fs::create_dir_all(tmp.path().join(d)).expect("mkdir");
        }
        tmp
    }

    #[test]
    fn folders_become_sites_with_detected_docroots() {
        let tmp = setup(&["blog/public", "My Shop", "my-shop", "!!!"]);
        let scan = scan(tmp.path(), &AppConfig::default());

        let domains: Vec<&str> = scan.sites.iter().map(|s| s.domain.as_str()).collect();
        // Sorted by folder name: "My Shop" comes before "blog" and claims the domain.
        assert_eq!(domains, ["my-shop.test", "blog.test"]);
        let blog = &scan.sites[1];
        assert_eq!(blog.docroot, tmp.path().join("blog").join("public"));
        assert_eq!(
            blog.docroot_source,
            DocrootSource::Detected(Some("public".into()))
        );
        assert!(blog.docroot_exists);

        assert_eq!(
            scan.skipped,
            [
                Skipped {
                    name: "!!!".into(),
                    reason: SkipReason::InvalidName
                },
                Skipped {
                    name: "my-shop".into(),
                    reason: SkipReason::DuplicateDomain {
                        domain: "my-shop.test".into(),
                        taken_by: "My Shop".into()
                    }
                },
            ]
        );
    }

    #[test]
    fn overrides_and_switches() {
        let tmp = setup(&["app/public", "app/dist", "old"]);
        let mut cfg = AppConfig::default();
        cfg.general.tld = "local".into();
        cfg.sites.insert(
            "app".into(),
            SiteOverride {
                enabled: true,
                docroot: "dist".into(),
            },
        );
        cfg.sites.insert(
            "old".into(),
            SiteOverride {
                enabled: false,
                docroot: String::new(),
            },
        );

        let scan = scan(tmp.path(), &cfg);
        assert_eq!(scan.sites.len(), 1);
        assert_eq!(scan.sites[0].domain, "app.local");
        assert_eq!(scan.sites[0].docroot, tmp.path().join("app").join("dist"));
        assert_eq!(scan.sites[0].docroot_source, DocrootSource::Configured);
        assert_eq!(scan.skipped[0].reason, SkipReason::Disabled);

        cfg.sites.get_mut("app").expect("app").docroot = "missing".into();
        assert!(!super::scan(tmp.path(), &cfg).sites[0].docroot_exists);

        cfg.sites.get_mut("app").expect("app").docroot = "x$y".into();
        assert_eq!(
            super::scan(tmp.path(), &cfg).skipped[0].reason,
            SkipReason::UnsafePath
        );

        cfg.general.auto_vhosts = false;
        assert_eq!(super::scan(tmp.path(), &cfg), Scan::default());
    }
}
