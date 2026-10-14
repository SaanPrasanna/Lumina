//! Filesystem helpers shared by every crate that persists files.

use std::io::Write;
use std::path::Path;

use crate::{Error, Result};

/// Writes `contents` to `path` atomically: a sibling temp file is written and
/// flushed to disk, then renamed over the target. Readers never see a partial
/// file, even if the app crashes or loses power mid-write.
pub fn write_atomic(path: &Path, contents: &[u8]) -> Result<()> {
    let io_err = |source| Error::Io {
        path: path.to_path_buf(),
        source,
    };

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(io_err)?;
    }

    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = Path::new(&tmp);

    let mut file = std::fs::File::create(tmp).map_err(io_err)?;
    file.write_all(contents).map_err(io_err)?;
    file.sync_all().map_err(io_err)?;
    drop(file);

    // `rename` replaces an existing target on every supported OS.
    std::fs::rename(tmp, path).map_err(io_err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_new_file_and_replaces_existing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("nested").join("file.toml");

        write_atomic(&path, b"one").expect("first write");
        write_atomic(&path, b"two").expect("second write");

        assert_eq!(std::fs::read(&path).expect("read"), b"two");
        assert!(
            !path.with_extension("toml.tmp").exists(),
            "temp file left behind"
        );
    }
}
