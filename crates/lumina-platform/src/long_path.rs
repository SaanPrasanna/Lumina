//! Paths longer than Windows' classic 260-character limit.
//!
//! Archives such as Node.js (`node_modules/npm/...`) contain deep paths. On
//! Windows an absolute path prefixed with `\\?\` bypasses the limit; elsewhere
//! paths are returned unchanged.

use std::path::{Path, PathBuf};

/// `path` in a form that file APIs accept even when it is very long.
pub fn long(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let s = path.as_os_str().to_string_lossy();
        if s.starts_with(r"\\?\") || !path.is_absolute() {
            return path.to_path_buf();
        }
        // The prefix disables `/` and `.` handling, so normalise first.
        let normal: PathBuf = path.components().collect();
        let text = normal.to_string_lossy().replace('/', "\\");
        if let Some(unc) = text.strip_prefix(r"\\") {
            return PathBuf::from(format!(r"\\?\UNC\{unc}"));
        }
        PathBuf::from(format!(r"\\?\{text}"))
    }
    #[cfg(not(windows))]
    {
        path.to_path_buf()
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn windows_paths_get_the_prefix() {
        assert_eq!(
            long(Path::new(r"C:\Lumina\bin")),
            PathBuf::from(r"\\?\C:\Lumina\bin")
        );
        assert_eq!(
            long(Path::new("C:/Lumina/bin/./x")),
            PathBuf::from(r"\\?\C:\Lumina\bin\x")
        );
        assert_eq!(long(Path::new(r"\\?\C:\a")), PathBuf::from(r"\\?\C:\a"));
        assert_eq!(long(Path::new("relative")), PathBuf::from("relative"));

        let tmp = tempfile::tempdir().expect("tempdir");
        let deep = (0..30).fold(tmp.path().to_path_buf(), |p, i| {
            p.join(format!("segment-{i:02}"))
        });
        assert!(deep.as_os_str().len() > 300);
        std::fs::create_dir_all(long(&deep)).expect("deep mkdir");
        std::fs::write(long(&deep.join("f.txt")), b"x").expect("deep write");
        std::fs::remove_dir_all(long(tmp.path())).expect("cleanup");
    }
}
