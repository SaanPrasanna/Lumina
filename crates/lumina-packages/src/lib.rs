//! Version manager for Nginx, Apache, PHP, Node, Python, MySQL, phpMyAdmin, ...
//!
//! Install = download (resumable) → SHA-256 check → extract into
//! `bin/<tool>/<version>.partial` → rename → record metadata. Every step
//! leaves nothing half-done behind: a failed check deletes the download, a
//! failed extraction deletes the partial folder.

pub mod catalog;
pub mod compat;
pub mod download;
pub mod extract;
pub mod installed;
pub mod verify;

use std::path::{Path, PathBuf};

use serde::Serialize;

pub use catalog::{Archive, Catalog, Kind, Package, Release};
pub use download::Cancel;
pub use installed::Installed;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{0}")]
    Catalog(String),
    #[error("download failed: {0}")]
    Download(String),
    #[error("{file} failed its checksum (expected {expected}, got {actual}); the file was deleted")]
    Checksum {
        file: String,
        expected: String,
        actual: String,
    },
    #[error("cannot extract: {0}")]
    Archive(String),
    #[error("cancelled")]
    Cancelled,
}

impl From<Error> for lumina_core::Error {
    fn from(e: Error) -> Self {
        match e {
            Error::Io { path, source } => lumina_core::Error::Io { path, source },
            other => lumina_core::Error::Other(other.to_string()),
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Stage {
    Downloading,
    Verifying,
    Extracting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub stage: Stage,
    pub received: u64,
    pub total: Option<u64>,
}

pub struct PackageManager {
    /// `<data-root>/bin`.
    bin: PathBuf,
    /// `<data-root>/tmp/downloads`; partial downloads stay here for resuming.
    downloads: PathBuf,
    catalog: Catalog,
    downloader: download::Downloader,
}

impl PackageManager {
    pub fn new(
        bin: PathBuf,
        downloads: PathBuf,
        catalog: Catalog,
        proxy: Option<&str>,
    ) -> Result<Self> {
        Ok(Self {
            bin,
            downloads,
            catalog,
            downloader: download::Downloader::new(proxy)?,
        })
    }

    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    /// `bin/<id>`.
    pub fn root(&self, id: &str) -> PathBuf {
        self.bin.join(id)
    }

    pub fn installed(&self, id: &str) -> Vec<Installed> {
        installed::list(&self.root(id))
    }

    /// Downloads, verifies and unpacks `id` `version` from the catalog.
    pub async fn install(
        &self,
        id: &str,
        version: &str,
        cancel: &Cancel,
        progress: &(dyn Fn(Progress) + Send + Sync),
    ) -> Result<Installed> {
        let release = self
            .catalog
            .release(id, version)
            .ok_or_else(|| {
                Error::Catalog(format!("{id} {version} is not available for this system"))
            })?
            .clone();
        let dest = self.root(id).join(&release.version);
        if dest.exists() {
            return Err(Error::Catalog(format!(
                "{id} {version} is already installed"
            )));
        }

        let archive = self.downloads.join(download_name(id, &release));
        if !archive.is_file() {
            let urls: Vec<&str> = release.urls().collect();
            self.downloader
                .fetch(&urls, &archive, cancel, &|received, total| {
                    progress(Progress {
                        stage: Stage::Downloading,
                        received,
                        total: total.or(release.size),
                    })
                })
                .await?;
        }

        progress(Progress {
            stage: Stage::Verifying,
            received: 0,
            total: None,
        });
        let (a, sha) = (archive.clone(), release.sha256.clone());
        blocking(move || verify::verify(&a, &sha)).await?;

        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        progress(Progress {
            stage: Stage::Extracting,
            received: 0,
            total: None,
        });
        let (a, r, d) = (archive.clone(), release.clone(), dest.clone());
        blocking(move || extract::extract(&a, &r, &d)).await?;
        installed::write_meta(&dest, id, &release)?;
        let _ = std::fs::remove_file(&archive);
        tracing::info!("installed {id} {} into {}", release.version, dest.display());

        Ok(Installed {
            version: release.version.clone(),
            dir: dest.clone(),
            meta: installed::list(&self.root(id))
                .into_iter()
                .find(|i| i.dir == dest)
                .and_then(|i| i.meta),
        })
    }

    /// Deletes `bin/<id>/<version>`. The caller checks it is not in use.
    pub fn uninstall(&self, id: &str, version: &str) -> Result<()> {
        if !catalog::is_safe_id(id) || !catalog::is_safe_id(version) {
            return Err(Error::Catalog(format!("bad package {id} {version}")));
        }
        let dir = self.root(id).join(version);
        if !dir.is_dir() {
            return Err(Error::Catalog(format!("{id} {version} is not installed")));
        }
        std::fs::remove_dir_all(lumina_platform::long_path::long(&dir)).map_err(|source| {
            Error::Io {
                path: dir.clone(),
                source,
            }
        })?;
        tracing::info!("removed {id} {version}");
        Ok(())
    }

    /// Size of an interrupted download that a new install would resume.
    pub fn partial_download(&self, id: &str, version: &str) -> Option<u64> {
        let release = self.catalog.release(id, version)?;
        let part = download::part_path(&self.downloads.join(download_name(id, release)));
        std::fs::metadata(part).ok().map(|m| m.len())
    }
}

/// `php-8.4.26-php-8.4.26-nts-Win32-vs17-x64.zip`-style names, unique per release.
fn download_name(id: &str, release: &Release) -> String {
    let file = release
        .url
        .rsplit('/')
        .next()
        .filter(|f| catalog::is_safe_id(f))
        .unwrap_or("download");
    format!("{id}-{}-{file}", release.version)
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| Error::Archive(format!("worker failed: {e}")))?
}

/// The bundled catalog next to the templates (`resources/catalog/packages.json`).
pub fn bundled_catalog(resources: &Path) -> Result<Catalog> {
    Catalog::load(&resources.join("catalog").join("packages.json"))
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;
    use crate::download::test_server;

    fn zip_bytes() -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buf);
            // Stored, so the archive is as big as its content and the cut
            // below really interrupts the download.
            let opts = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            zip.start_file("tool-1.0/tool.exe", opts).expect("start");
            zip.write_all(&vec![7u8; 200_000]).expect("write");
            zip.finish().expect("finish");
        }
        buf.into_inner()
    }

    fn catalog(url: &str, sha256: &str) -> Catalog {
        Catalog {
            schema_version: 1,
            updated: String::new(),
            packages: vec![Package {
                id: "tool".into(),
                name: "Tool".into(),
                kind: Kind::Tool,
                description: String::new(),
                homepage: String::new(),
                versions: vec![Release {
                    version: "1.0".into(),
                    os: "any".into(),
                    arch: "any".into(),
                    url: url.into(),
                    mirrors: vec![],
                    sha256: sha256.into(),
                    size: None,
                    archive: Archive::Zip,
                    strip: 1,
                    subdir: None,
                    file_name: None,
                    channel: None,
                    requires: Default::default(),
                    checksum_source: None,
                }],
            }],
        }
    }

    fn manager(tmp: &Path, url: &str, sha: &str) -> PackageManager {
        PackageManager::new(
            tmp.join("bin"),
            tmp.join("downloads"),
            catalog(url, sha),
            None,
        )
        .expect("manager")
    }

    #[tokio::test]
    async fn installs_verifies_and_uninstalls() {
        let body = zip_bytes();
        let sha = {
            use sha2::Digest;
            sha2::Sha256::digest(&body)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        };
        let server = test_server::serve(body, Some(60_000), true).await;
        let url = format!("http://{}/tool.zip", server.addr);
        let tmp = tempfile::tempdir().expect("tempdir");
        let pm = manager(tmp.path(), &url, &sha);
        let cancel = Cancel::default();

        // The first attempt is cut off; the second resumes and installs.
        assert!(pm.install("tool", "1.0", &cancel, &|_| {}).await.is_err());
        assert_eq!(pm.partial_download("tool", "1.0"), Some(60_000));
        let stages = std::sync::Mutex::new(Vec::new());
        let installed = pm
            .install("tool", "1.0", &cancel, &|p| {
                stages.lock().expect("lock").push(p.stage)
            })
            .await
            .expect("install");
        assert!(installed.dir.join("tool.exe").is_file());
        assert_eq!(
            installed.meta.as_ref().map(|m| m.sha256.as_str()),
            Some(sha.as_str())
        );
        let stages = stages.into_inner().expect("stages");
        assert!(stages.contains(&Stage::Verifying) && stages.ends_with(&[Stage::Extracting]));
        assert_eq!(pm.installed("tool").len(), 1);
        assert!(
            pm.install("tool", "1.0", &cancel, &|_| {}).await.is_err(),
            "already installed"
        );

        pm.uninstall("tool", "1.0").expect("uninstall");
        assert!(pm.installed("tool").is_empty());
        assert!(pm.uninstall("tool", "../bin").is_err());
    }

    #[tokio::test]
    async fn tampered_archive_is_rejected_and_deleted() {
        let server = test_server::serve(zip_bytes(), None, true).await;
        let url = format!("http://{}/tool.zip", server.addr);
        let tmp = tempfile::tempdir().expect("tempdir");
        let pm = manager(tmp.path(), &url, &"ab".repeat(32));

        let err = pm
            .install("tool", "1.0", &Cancel::default(), &|_| {})
            .await
            .expect_err("bad hash");
        assert!(matches!(err, Error::Checksum { .. }), "{err}");
        assert!(pm.installed("tool").is_empty(), "nothing may be installed");
        let leftovers = std::fs::read_dir(tmp.path().join("downloads"))
            .expect("dir")
            .count();
        assert_eq!(leftovers, 0, "the bad download must be deleted");
    }
}
