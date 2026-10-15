//! zip / tar.gz extraction into bin/<tool>/<version>.
//!
//! Files go to `<version>.partial/` first and the folder is renamed when
//! everything is written, so a half-extracted version never looks installed.
//! Entries that would land outside the target (`../`, absolute paths) are
//! refused.

use std::io::Read;
use std::path::{Component, Path, PathBuf};

use lumina_platform::long_path::long;

use crate::catalog::{Archive, Release};
use crate::{Error, Result};

/// `<dest>.partial`.
pub fn partial_path(dest: &Path) -> PathBuf {
    let mut name = dest.as_os_str().to_owned();
    name.push(".partial");
    PathBuf::from(name)
}

/// Where an entry goes, or `None` if it is skipped (outside `subdir`, a
/// stripped directory). Errors on paths that escape the target.
fn target(entry: &Path, strip: u32, subdir: Option<&str>) -> Result<Option<PathBuf>> {
    let mut parts = Vec::new();
    for c in entry.components() {
        match c {
            Component::Normal(p) => parts.push(p.to_owned()),
            Component::CurDir => {}
            _ => {
                return Err(Error::Archive(format!(
                    "unsafe path in archive: {}",
                    entry.display()
                )));
            }
        }
    }
    let mut rest: &[std::ffi::OsString] = &parts;
    if let Some(sub) = subdir {
        // Find the folder anywhere in the leading components.
        let Some(pos) = rest.iter().position(|p| p == sub) else {
            return Ok(None);
        };
        rest = &rest[pos + 1..];
    } else {
        let n = strip as usize;
        if rest.len() <= n {
            return Ok(None);
        }
        rest = &rest[n..];
    }
    if rest.is_empty() {
        return Ok(None);
    }
    Ok(Some(rest.iter().collect()))
}

/// Unpacks `archive` (downloaded for `release`) into `dest`, which must not
/// exist yet.
pub fn extract(archive: &Path, release: &Release, dest: &Path) -> Result<()> {
    let partial = partial_path(dest);
    if partial.exists() {
        std::fs::remove_dir_all(long(&partial)).map_err(|source| Error::Io {
            path: partial.clone(),
            source,
        })?;
    }
    std::fs::create_dir_all(long(&partial)).map_err(|source| Error::Io {
        path: partial.clone(),
        source,
    })?;

    let result = match release.archive {
        Archive::Zip => unzip(archive, release, &partial),
        Archive::TarGz | Archive::TarXz => untar(archive, release, &partial),
        Archive::File => {
            let name = release
                .file_name
                .as_deref()
                .ok_or_else(|| Error::Archive("a single-file release needs file_name".into()))?;
            if !crate::catalog::is_safe_id(name) {
                return Err(Error::Archive(format!("bad file name {name:?}")));
            }
            std::fs::copy(archive, partial.join(name))
                .map(|_| ())
                .map_err(|source| Error::Io {
                    path: partial.join(name),
                    source,
                })
        }
    };
    if let Err(err) = result {
        let _ = std::fs::remove_dir_all(long(&partial));
        return Err(err);
    }
    let empty = std::fs::read_dir(&partial)
        .map(|mut d| d.next().is_none())
        .unwrap_or(true);
    if empty {
        let _ = std::fs::remove_dir_all(long(&partial));
        return Err(Error::Archive(
            "nothing was extracted (wrong strip/subdir?)".into(),
        ));
    }
    std::fs::rename(&partial, dest).map_err(|source| Error::Io {
        path: dest.to_path_buf(),
        source,
    })?;
    // Verified by checksum: Gatekeeper must not block the unsigned runtimes.
    lumina_platform::quarantine::clear(dest);
    Ok(())
}

fn write_entry(out: &Path, reader: &mut impl Read, mode: Option<u32>) -> Result<()> {
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(long(parent)).map_err(|source| Error::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let mut file = std::fs::File::create(long(out)).map_err(|source| Error::Io {
        path: out.to_path_buf(),
        source,
    })?;
    std::io::copy(reader, &mut file).map_err(|source| Error::Io {
        path: out.to_path_buf(),
        source,
    })?;
    #[cfg(unix)]
    if let Some(mode) = mode {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(out, std::fs::Permissions::from_mode(mode & 0o755));
    }
    #[cfg(not(unix))]
    let _ = mode;
    Ok(())
}

fn unzip(archive: &Path, release: &Release, dest: &Path) -> Result<()> {
    let file = std::fs::File::open(archive).map_err(|source| Error::Io {
        path: archive.to_path_buf(),
        source,
    })?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| Error::Archive(e.to_string()))?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| Error::Archive(e.to_string()))?;
        // `enclosed_name` already rejects `..` and absolute names.
        let name = entry
            .enclosed_name()
            .ok_or_else(|| Error::Archive(format!("unsafe path in archive: {}", entry.name())))?;
        let Some(rel) = target(&name, release.strip, release.subdir.as_deref())? else {
            continue;
        };
        let out = dest.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(long(&out)).map_err(|source| Error::Io {
                path: out.clone(),
                source,
            })?;
            continue;
        }
        let mode = entry.unix_mode();
        write_entry(&out, &mut entry, mode)?;
    }
    Ok(())
}

fn untar(archive: &Path, release: &Release, dest: &Path) -> Result<()> {
    let file = std::fs::File::open(archive).map_err(|source| Error::Io {
        path: archive.to_path_buf(),
        source,
    })?;
    let reader: Box<dyn Read> = match release.archive {
        Archive::TarXz => Box::new(lzma_rust2::XzReader::new(
            std::io::BufReader::new(file),
            true,
        )),
        _ => Box::new(flate2::read::GzDecoder::new(file)),
    };
    let mut tar = tar::Archive::new(reader);
    let entries = tar.entries().map_err(|e| Error::Archive(e.to_string()))?;
    for entry in entries {
        let mut entry = entry.map_err(|e| Error::Archive(e.to_string()))?;
        let name = entry
            .path()
            .map_err(|e| Error::Archive(e.to_string()))?
            .into_owned();
        let Some(rel) = target(&name, release.strip, release.subdir.as_deref())? else {
            continue;
        };
        let out = dest.join(rel);
        let kind = entry.header().entry_type();
        if kind.is_dir() {
            std::fs::create_dir_all(&out).map_err(|source| Error::Io {
                path: out.clone(),
                source,
            })?;
        } else if kind.is_symlink() {
            // Node.js ships bin/npm -> ../lib/node_modules/npm/bin/npm-cli.js.
            let link = entry
                .link_name()
                .map_err(|e| Error::Archive(e.to_string()))?
                .ok_or_else(|| Error::Archive("symlink without target".into()))?
                .into_owned();
            if link.is_absolute()
                || link
                    .components()
                    .filter(|c| *c == Component::ParentDir)
                    .count()
                    > 3
            {
                return Err(Error::Archive(format!(
                    "unsafe link {} -> {}",
                    name.display(),
                    link.display()
                )));
            }
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent).map_err(|source| Error::Io {
                    path: parent.to_path_buf(),
                    source,
                })?;
            }
            #[cfg(unix)]
            std::os::unix::fs::symlink(&link, &out).map_err(|source| Error::Io {
                path: out.clone(),
                source,
            })?;
        } else if kind.is_file() {
            let mode = entry.header().mode().ok();
            write_entry(&out, &mut entry, mode)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;
    use crate::catalog::Requires;

    fn release(archive: Archive, strip: u32, subdir: Option<&str>) -> Release {
        Release {
            version: "1.0".into(),
            os: "any".into(),
            arch: "any".into(),
            url: "https://example.invalid/a".into(),
            mirrors: vec![],
            sha256: "0".repeat(64),
            size: None,
            archive,
            strip,
            subdir: subdir.map(str::to_owned),
            file_name: Some("tool.phar".into()),
            channel: None,
            requires: Requires::default(),
            checksum_source: None,
        }
    }

    fn make_zip(path: &Path, entries: &[(&str, &str)]) {
        let file = std::fs::File::create(path).expect("create");
        let mut zip = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default();
        for (name, data) in entries {
            if name.ends_with('/') {
                zip.add_directory(*name, opts).expect("dir");
            } else {
                zip.start_file(*name, opts).expect("start");
                zip.write_all(data.as_bytes()).expect("write");
            }
        }
        zip.finish().expect("finish");
    }

    #[test]
    fn strip_and_subdir() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let z = tmp.path().join("a.zip");
        make_zip(
            &z,
            &[
                ("nginx-1.0/", ""),
                ("nginx-1.0/nginx.exe", "exe"),
                ("nginx-1.0/conf/nginx.conf", "conf"),
            ],
        );
        let dest = tmp.path().join("bin/nginx/1.0");
        extract(&z, &release(Archive::Zip, 1, None), &dest).expect("extract");
        assert_eq!(
            std::fs::read_to_string(dest.join("nginx.exe")).expect("exe"),
            "exe"
        );
        assert!(dest.join("conf/nginx.conf").is_file());
        assert!(!partial_path(&dest).exists());

        let z2 = tmp.path().join("b.zip");
        make_zip(
            &z2,
            &[
                ("ReadMe.txt", "x"),
                ("Apache24/bin/httpd.exe", "h"),
                ("Apache24/conf/httpd.conf", "c"),
            ],
        );
        let dest2 = tmp.path().join("bin/apache/2.4");
        extract(&z2, &release(Archive::Zip, 0, Some("Apache24")), &dest2).expect("subdir");
        assert!(dest2.join("bin/httpd.exe").is_file());
        assert!(!dest2.join("ReadMe.txt").exists());
    }

    #[test]
    fn single_files_and_tar_gz() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let phar = tmp.path().join("download");
        std::fs::write(&phar, "<?php").expect("write");
        let dest = tmp.path().join("bin/composer/2.0");
        extract(&phar, &release(Archive::File, 0, None), &dest).expect("file");
        assert!(dest.join("tool.phar").is_file());

        let tgz = tmp.path().join("node.tar.gz");
        {
            let gz = flate2::write::GzEncoder::new(
                std::fs::File::create(&tgz).expect("c"),
                flate2::Compression::fast(),
            );
            let mut tar = tar::Builder::new(gz);
            let mut header = tar::Header::new_gnu();
            header.set_size(4);
            header.set_mode(0o755);
            header.set_cksum();
            tar.append_data(&mut header, "node-v1/bin/node", &b"node"[..])
                .expect("append");
            tar.into_inner().expect("tar").finish().expect("gz");
        }
        let dest = tmp.path().join("bin/node/1");
        extract(&tgz, &release(Archive::TarGz, 1, None), &dest).expect("tar");
        assert_eq!(std::fs::read(dest.join("bin/node")).expect("node"), b"node");
    }

    #[test]
    fn tar_xz() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let txz = tmp.path().join("mysql.tar.xz");
        {
            let xz = lzma_rust2::XzWriter::new(
                std::fs::File::create(&txz).expect("c"),
                lzma_rust2::XzOptions::with_preset(1),
            )
            .expect("xz");
            let mut tar = tar::Builder::new(xz);
            let mut header = tar::Header::new_gnu();
            header.set_size(6);
            header.set_mode(0o755);
            header.set_cksum();
            tar.append_data(&mut header, "mysql-9.7.2/bin/mysqld", &b"mysqld"[..])
                .expect("append");
            tar.into_inner().expect("tar").finish().expect("xz");
        }
        let dest = tmp.path().join("bin/mysql/9.7.2");
        extract(&txz, &release(Archive::TarXz, 1, None), &dest).expect("tar.xz");
        assert_eq!(std::fs::read(dest.join("bin/mysqld")).expect("mysqld"), b"mysqld");
    }

    #[test]
    fn traversal_is_refused_and_nothing_is_left_behind() {
        assert!(target(Path::new("../evil"), 0, None).is_err());
        assert!(target(Path::new("/etc/passwd"), 0, None).is_err());
        assert_eq!(
            target(Path::new("a/b"), 1, None).expect("ok"),
            Some(PathBuf::from("b"))
        );
        assert_eq!(target(Path::new("a"), 1, None).expect("ok"), None);

        let tmp = tempfile::tempdir().expect("tempdir");
        let z = tmp.path().join("empty.zip");
        make_zip(&z, &[("only-top/", "")]);
        let dest = tmp.path().join("bin/x/1");
        assert!(extract(&z, &release(Archive::Zip, 1, None), &dest).is_err());
        assert!(!dest.exists() && !partial_path(&dest).exists());
    }
}
