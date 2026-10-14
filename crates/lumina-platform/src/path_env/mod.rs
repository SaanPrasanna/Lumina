//! Add/remove bundled PHP, Node, Python, Composer to the user PATH (registry on Windows, shell rc files on macOS/Linux).
//!
//! Lumina adds one folder, `<data-root>/bin/.shims`, at the front of the
//! user's PATH; the shims inside follow the active versions, so PATH never
//! needs editing again.
//!
//! - Windows: `HKCU\Environment\Path` gets `<dir>;` prepended (type kept, or
//!   `REG_EXPAND_SZ`), then `WM_SETTINGCHANGE "Environment"` is broadcast so
//!   newly started terminals see it. Removing strips exactly that prefix.
//! - macOS/Linux: a marker block in the shell start-up files (and a fish
//!   `conf.d` file), removed exactly on "Remove".

use std::path::{Path, PathBuf};

use serde::Serialize;

#[cfg(unix)]
mod unix;
#[cfg(target_os = "windows")]
mod windows;

pub const BLOCK_BEGIN: &str = "# >>> lumina >>>";
pub const BLOCK_END: &str = "# <<< lumina <<<";

/// Commands whose resolution the status reports.
pub const CHECKED_COMMANDS: [&str; 4] = ["php", "composer", "node", "python"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shadow {
    pub command: String,
    /// Where a new terminal would find `command` before Lumina's folder.
    pub found: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathStatus {
    /// Lumina's folder is in the user's PATH (registry / start-up files).
    pub added: bool,
    /// Lumina's folder is in the machine-wide PATH (Windows, via the helper).
    pub added_system: bool,
    /// Start-up files that contain Lumina's block (macOS/Linux).
    pub files: Vec<PathBuf>,
    /// Commands that resolve to something else first (e.g. a system-wide PHP
    /// listed in the machine PATH, which Windows puts before the user PATH).
    pub shadowed: Vec<Shadow>,
}

// ---------------------------------------------------------------- Windows text

fn same_dir(a: &str, b: &str) -> bool {
    let norm = |s: &str| s.trim().trim_end_matches(['\\', '/']).to_ascii_lowercase();
    norm(a) == norm(b)
}

/// `raw` with `dir` first. Unchanged if `dir` is already the first entry.
pub fn prepend_entry(raw: &str, dir: &str) -> String {
    if raw
        .split(';')
        .next()
        .is_some_and(|first| same_dir(first, dir))
    {
        return raw.to_owned();
    }
    if raw.is_empty() {
        dir.to_owned()
    } else {
        format!("{dir};{raw}")
    }
}

/// `raw` without `dir`. Undoes [`prepend_entry`] exactly (the prefix is
/// stripped as added); entries elsewhere are removed one by one.
pub fn remove_entry(raw: &str, dir: &str) -> String {
    if let Some(rest) = raw.strip_prefix(&format!("{dir};")) {
        return remove_entry(rest, dir);
    }
    if same_dir(raw, dir) {
        return String::new();
    }
    if !raw.split(';').any(|e| same_dir(e, dir)) {
        return raw.to_owned();
    }
    raw.split(';')
        .filter(|e| !same_dir(e, dir))
        .collect::<Vec<_>>()
        .join(";")
}

pub fn contains_entry(raw: &str, dir: &str) -> bool {
    raw.split(';').any(|e| same_dir(e, dir))
}

// ------------------------------------------------------------ start-up files

/// `text` with Lumina's block (at the end) setting `body`; an existing block
/// is replaced in place.
pub fn add_block(text: &str, body: &str) -> String {
    let block = format!("{BLOCK_BEGIN}\n{body}\n{BLOCK_END}\n");
    if let Some((start, end)) = find_block(text) {
        return format!("{}{block}{}", &text[..start], &text[end..]);
    }
    let mut out = text.to_owned();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&block);
    out
}

/// `text` without Lumina's block; byte-identical to the text before
/// [`add_block`] when that text ended with a newline.
pub fn remove_block(text: &str) -> String {
    match find_block(text) {
        Some((start, end)) => format!("{}{}", &text[..start], &text[end..]),
        None => text.to_owned(),
    }
}

/// Byte range of the block including its final newline.
fn find_block(text: &str) -> Option<(usize, usize)> {
    let start = text.find(BLOCK_BEGIN)?;
    let end_marker = start + text[start..].find(BLOCK_END)?;
    let mut end = end_marker + BLOCK_END.len();
    if text[end..].starts_with("\r\n") {
        end += 2;
    } else if text[end..].starts_with('\n') {
        end += 1;
    }
    Some((start, end))
}

// ------------------------------------------------------------------- public

/// Puts `dir` first on the user's PATH. Returns the files or keys changed.
pub fn add(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    #[cfg(target_os = "windows")]
    return windows::add(dir);
    #[cfg(unix)]
    return unix::add(dir);
}

/// Takes `dir` off the user's PATH again.
pub fn remove(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    #[cfg(target_os = "windows")]
    return windows::remove(dir);
    #[cfg(unix)]
    return unix::remove(dir);
}

/// Whether `dir` is Lumina's shim folder (`...\bin\.shims`), the only folder
/// the elevated helper may put on the machine PATH.
pub fn is_shim_dir(dir: &Path) -> bool {
    let text = dir.to_string_lossy();
    dir.is_absolute()
        && !text.contains([';', '%', '"', '\n', '\r'])
        && dir.file_name().is_some_and(|n| n == ".shims")
        && dir
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|n| n == "bin")
}

/// Puts `dir` first on the machine-wide PATH (Windows; needs administrator
/// rights, so `lumina-helper` calls it).
pub fn add_system(dir: &Path) -> std::io::Result<bool> {
    #[cfg(target_os = "windows")]
    return windows::add_system(dir);
    #[cfg(unix)]
    {
        let _ = dir;
        Err(std::io::Error::other(
            "a machine-wide PATH is only edited on Windows",
        ))
    }
}

/// Takes `dir` off the machine-wide PATH (Windows, administrator).
pub fn remove_system(dir: &Path) -> std::io::Result<bool> {
    #[cfg(target_os = "windows")]
    return windows::remove_system(dir);
    #[cfg(unix)]
    {
        let _ = dir;
        Ok(false)
    }
}

pub fn status(dir: &Path) -> PathStatus {
    #[cfg(target_os = "windows")]
    return windows::status(dir);
    #[cfg(unix)]
    return unix::status(dir);
}

/// The first `command` on `dirs`, trying the platform's executable
/// extensions (`PATHEXT` on Windows).
pub fn find_in(dirs: &[PathBuf], command: &str) -> Option<PathBuf> {
    let exts: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into())
            .split(';')
            .filter(|e| !e.is_empty())
            .map(|e| e.to_ascii_lowercase())
            .collect()
    } else {
        vec![String::new()]
    };
    dirs.iter().find_map(|dir| {
        exts.iter()
            .map(|ext| dir.join(format!("{command}{ext}")))
            .find(|p| p.is_file())
    })
}

/// The folder an executable on `PATH` lives in, for a command name such as
/// `code` (which is `code.cmd` on Windows). Uses this process's PATH.
pub fn which(command: &str) -> Option<PathBuf> {
    let dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    find_in(&dirs, command)
}

/// Commands among [`CHECKED_COMMANDS`] that `dirs` (in lookup order) would
/// resolve somewhere other than `ours`.
pub fn shadows(dirs: &[PathBuf], ours: &Path) -> Vec<Shadow> {
    CHECKED_COMMANDS
        .iter()
        .filter_map(|cmd| {
            let found = find_in(dirs, cmd)?;
            let in_ours = found
                .parent()
                .is_some_and(|p| same_dir(&p.to_string_lossy(), &ours.to_string_lossy()));
            // Only a problem if Lumina provides the command too.
            let we_have = find_in(&[ours.to_path_buf()], cmd).is_some();
            (!in_ours && we_have).then(|| Shadow {
                command: (*cmd).to_owned(),
                found,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_prefix_round_trips_exactly() {
        let dir = r"C:\Lumina\bin\.shims";
        for raw in [
            r"%USERPROFILE%\AppData\Local\Microsoft\WindowsApps;C:\Users\me\.cargo\bin;",
            r"C:\a;;C:\b",
            "",
            r"C:\only",
        ] {
            let added = prepend_entry(raw, dir);
            assert!(
                added.starts_with(dir) && contains_entry(&added, dir),
                "{added}"
            );
            assert_eq!(
                prepend_entry(&added, dir),
                added,
                "adding twice changes nothing"
            );
            assert_eq!(
                remove_entry(&added, dir),
                raw,
                "remove restores {raw:?} exactly"
            );
        }
        // Entries the user moved elsewhere are removed too; the rest is kept.
        assert_eq!(
            remove_entry(r"C:\a;C:\Lumina\bin\.shims\;C:\b", dir),
            r"C:\a;C:\b"
        );
        assert_eq!(remove_entry(r"C:\a;C:\b", dir), r"C:\a;C:\b");
    }

    #[test]
    fn rc_block_round_trips_exactly() {
        let body = r#"export PATH="$HOME/Lumina/bin/.shims:$PATH""#;
        for original in ["", "alias ll='ls -l'\n", "# zshrc\nexport EDITOR=vim\n\n"] {
            let added = add_block(original, body);
            assert!(added.contains(BLOCK_BEGIN) && added.contains(body));
            assert_eq!(
                add_block(&added, body),
                added,
                "adding twice changes nothing"
            );
            assert_eq!(remove_block(&added), original);
        }
        // A changed folder replaces the block in place.
        let added = add_block("a\n", "old");
        let replaced = add_block(&format!("{added}b\n"), "new");
        assert_eq!(replaced, format!("a\n{BLOCK_BEGIN}\nnew\n{BLOCK_END}\nb\n"));
        // No trailing newline: one is added before the block.
        assert_eq!(remove_block(&add_block("x", body)), "x\n");
    }

    #[test]
    fn shadow_detection() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let ours = tmp.path().join("shims");
        let other = tmp.path().join("other-php");
        std::fs::create_dir_all(&ours).expect("mkdir");
        std::fs::create_dir_all(&other).expect("mkdir");
        let exe = if cfg!(windows) { ".exe" } else { "" };
        std::fs::write(ours.join(format!("php{exe}")), "").expect("w");
        std::fs::write(ours.join(format!("node{exe}")), "").expect("w");
        std::fs::write(other.join(format!("php{exe}")), "").expect("w");

        let s = shadows(&[other.clone(), ours.clone()], &ours);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].command, "php");
        assert!(shadows(&[ours.clone(), other], &ours).is_empty());
    }

    #[test]
    fn only_the_shim_folder_may_go_on_the_machine_path() {
        let ok = if cfg!(windows) {
            r"C:\Lumina\bin\.shims"
        } else {
            "/home/a/Lumina/bin/.shims"
        };
        assert!(is_shim_dir(Path::new(ok)));
        let bad: &[&str] = if cfg!(windows) {
            &[
                r"C:\Windows\System32",
                r"C:\Lumina\bin",
                r"bin\.shims",
                r"C:\x;C:\evil\bin\.shims",
                r"C:\%TEMP%\bin\.shims",
            ]
        } else {
            &["/usr/bin", "bin/.shims", "/x;y/bin/.shims"]
        };
        for b in bad {
            assert!(!is_shim_dir(Path::new(b)), "{b}");
        }
    }
}
