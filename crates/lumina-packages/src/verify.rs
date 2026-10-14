//! SHA-256 verification of downloaded archives.

use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::{Error, Result};

/// Lowercase hex SHA-256 of a file.
pub fn sha256_file(path: &Path) -> Result<String> {
    let io = |source| Error::Io {
        path: path.to_path_buf(),
        source,
    };
    let mut file = std::fs::File::open(path).map_err(io)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = file.read(&mut buf).map_err(io)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// Checks `path` against `expected`; a mismatch deletes the file, so a
/// tampered or corrupt download is never extracted or resumed.
pub fn verify(path: &Path, expected: &str) -> Result<()> {
    let actual = sha256_file(path)?;
    if actual.eq_ignore_ascii_case(expected.trim()) {
        return Ok(());
    }
    let _ = std::fs::remove_file(path);
    Err(Error::Checksum {
        file: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        expected: expected.to_ascii_lowercase(),
        actual,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_hash_passes_and_a_mismatch_deletes_the_file() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let file = tmp.path().join("a.zip");
        std::fs::write(&file, b"abc").expect("write");
        let abc = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert_eq!(sha256_file(&file).expect("hash"), abc);
        verify(&file, &abc.to_uppercase()).expect("case-insensitive match");

        std::fs::write(&file, b"abd").expect("tamper");
        let err = verify(&file, abc).expect_err("tampered");
        assert!(matches!(err, Error::Checksum { .. }), "{err}");
        assert!(!file.exists(), "a bad download must be deleted");
    }
}
