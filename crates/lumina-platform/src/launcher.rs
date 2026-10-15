//! Open folders, terminals and the user's custom editor (VS Code, PhpStorm, Notepad++...) at a path.
//!
//! Everything is started detached: closing Lumina (which ends the services'
//! process groups) never closes the user's editor or terminal.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Serialize;

use crate::path_env;

/// A known editor found on this machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EditorPreset {
    pub id: &'static str,
    pub name: &'static str,
    pub command: PathBuf,
    /// Arguments to open a folder or file: `{path}`.
    pub args: Vec<String>,
    /// Arguments to open a file at a line: `{path}`, `{line}`.
    pub args_line: Vec<String>,
}

struct Known {
    id: &'static str,
    name: &'static str,
    /// Command names to look up on PATH.
    on_path: &'static [&'static str],
    /// Install locations, relative to a base folder (see `bases`).
    windows: &'static [&'static str],
    macos: &'static [&'static str],
    /// To open a folder or file.
    args: &'static [&'static str],
    args_line: &'static [&'static str],
}

const KNOWN: &[Known] = &[
    Known {
        id: "vscode",
        name: "VS Code",
        on_path: &["code"],
        windows: &[
            r"%LOCALAPPDATA%\Programs\Microsoft VS Code\bin\code.cmd",
            r"%ProgramFiles%\Microsoft VS Code\bin\code.cmd",
        ],
        macos: &["/Applications/Visual Studio Code.app/Contents/Resources/app/bin/code"],
        args: &["{path}"],
        args_line: &["--goto", "{path}:{line}"],
    },
    Known {
        id: "vscode-insiders",
        name: "VS Code Insiders",
        on_path: &["code-insiders"],
        windows: &[r"%LOCALAPPDATA%\Programs\Microsoft VS Code Insiders\bin\code-insiders.cmd"],
        macos: &["/Applications/Visual Studio Code - Insiders.app/Contents/Resources/app/bin/code"],
        args: &["{path}"],
        args_line: &["--goto", "{path}:{line}"],
    },
    Known {
        id: "cursor",
        name: "Cursor",
        on_path: &["cursor"],
        windows: &[
            r"%LOCALAPPDATA%\Programs\cursor\resources\app\bin\cursor.cmd",
            r"%LOCALAPPDATA%\Programs\cursor\Cursor.exe",
        ],
        macos: &["/Applications/Cursor.app/Contents/Resources/app/bin/cursor"],
        args: &["{path}"],
        args_line: &["--goto", "{path}:{line}"],
    },
    Known {
        id: "phpstorm",
        name: "PhpStorm",
        on_path: &["phpstorm", "pstorm"],
        windows: &[r"%LOCALAPPDATA%\JetBrains\Toolbox\scripts\phpstorm.cmd"],
        macos: &["/Applications/PhpStorm.app/Contents/MacOS/phpstorm"],
        args: &["{path}"],
        args_line: &["--line", "{line}", "{path}"],
    },
    Known {
        id: "sublime",
        name: "Sublime Text",
        on_path: &["subl"],
        windows: &[
            r"%ProgramFiles%\Sublime Text\subl.exe",
            r"%ProgramFiles%\Sublime Text 3\subl.exe",
        ],
        macos: &["/Applications/Sublime Text.app/Contents/SharedSupport/bin/subl"],
        args: &["{path}"],
        args_line: &["{path}:{line}"],
    },
    Known {
        id: "notepadpp",
        name: "Notepad++",
        on_path: &["notepad++"],
        windows: &[
            r"%ProgramFiles%\Notepad++\notepad++.exe",
            r"%ProgramFiles(x86)%\Notepad++\notepad++.exe",
        ],
        macos: &[],
        args: &["-openFoldersAsWorkspace", "{path}"],
        args_line: &["-n{line}", "{path}"],
    },
    Known {
        id: "zed",
        name: "Zed",
        on_path: &["zed", "zeditor"],
        windows: &[
            r"%LOCALAPPDATA%\Programs\Zed\bin\zed.exe",
            r"%LOCALAPPDATA%\Programs\Zed\Zed.exe",
        ],
        macos: &["/Applications/Zed.app/Contents/MacOS/cli"],
        args: &["{path}"],
        args_line: &["{path}:{line}"],
    },
];

/// `%VAR%` references expanded from the environment; `None` if one is unset.
fn expand_vars(s: &str) -> Option<PathBuf> {
    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let end = after.find('%')?;
        out.push_str(&std::env::var(&after[..end]).ok()?);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Some(PathBuf::from(out))
}

/// Where `k` is installed on this machine: its usual install folder, else PATH.
fn installed_at(k: &Known) -> Option<PathBuf> {
    let places = if cfg!(windows) {
        k.windows
    } else if cfg!(target_os = "macos") {
        k.macos
    } else {
        &[]
    };
    places
        .iter()
        .filter_map(|p| expand_vars(p))
        .find(|p| p.is_file())
        .or_else(|| k.on_path.iter().find_map(|c| path_env::which(c)))
}

/// Editors installed on this machine, in a fixed order.
pub fn editor_presets() -> Vec<EditorPreset> {
    KNOWN
        .iter()
        .filter_map(|k| {
            Some(EditorPreset {
                id: k.id,
                name: k.name,
                command: installed_at(k)?,
                args: k.args.iter().map(|a| (*a).to_owned()).collect(),
                args_line: k.args_line.iter().map(|a| (*a).to_owned()).collect(),
            })
        })
        .collect()
}

/// The arguments for opening `path` (at `line` when given and the editor
/// supports it).
pub fn editor_args(
    args: &[String],
    args_line: &[String],
    path: &Path,
    line: Option<u32>,
) -> Vec<String> {
    let path = path.to_string_lossy();
    let template = match line {
        Some(_) if !args_line.is_empty() => args_line,
        _ => args,
    };
    let line = line.map(|l| l.to_string()).unwrap_or_default();
    let mut out: Vec<String> = template
        .iter()
        .map(|a| a.replace("{path}", &path).replace("{line}", &line))
        .collect();
    if !template.iter().any(|a| a.contains("{path}")) {
        out.push(path.into_owned());
    }
    out
}

/// Resolves a command name or path: absolute paths as they are, names on
/// PATH (including `code.cmd`-style launchers on Windows), then a known
/// editor's install folder (`notepad++` is usually not on PATH).
pub fn resolve(command: &str) -> Option<PathBuf> {
    let command = command.trim();
    let p = Path::new(command);
    if p.is_absolute() {
        return p.is_file().then(|| p.to_path_buf());
    }
    path_env::which(command).or_else(|| {
        KNOWN
            .iter()
            .find(|k| k.on_path.iter().any(|c| c.eq_ignore_ascii_case(command)))
            .and_then(installed_at)
    })
}

/// Starts `program` detached from Lumina. `path_prefix` goes in front of PATH.
pub fn spawn_detached(
    program: &Path,
    args: &[String],
    cwd: Option<&Path>,
    path_prefix: Option<&Path>,
) -> std::io::Result<()> {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    if let Some(prefix) = path_prefix {
        cmd.env("PATH", prefixed_path(prefix));
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        // `.cmd` launchers (code.cmd) need a console but must not show one.
        let is_script = program
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat"));
        let flags = if is_script {
            CREATE_NO_WINDOW
        } else {
            DETACHED_PROCESS
        };
        // Leave any job object Lumina itself runs in (a launcher or terminal
        // may have started it in one), so closing Lumina never closes the
        // editor. Jobs that forbid breaking away refuse the flag; then the
        // editor is started normally.
        const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
        cmd.creation_flags(flags | CREATE_NEW_PROCESS_GROUP | CREATE_BREAKAWAY_FROM_JOB);
        if cmd.spawn().is_ok() {
            return Ok(());
        }
        cmd.creation_flags(flags | CREATE_NEW_PROCESS_GROUP);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    cmd.spawn().map(|_| ())
}

fn prefixed_path(prefix: &Path) -> OsString {
    let mut dirs = vec![prefix.to_path_buf()];
    if let Some(existing) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&existing).filter(|d| d != prefix));
    }
    std::env::join_paths(dirs).unwrap_or_default()
}

/// Opens a terminal in `dir` whose PATH starts with `path_prefix` (Lumina's
/// shim folder), so `php`, `composer`, `node` work there even before
/// "Add to PATH".
pub fn open_terminal(dir: &Path, path_prefix: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
        let set_path = format!("set \"PATH={};%PATH%\"", path_prefix.display());
        // Windows Terminal hands the command to its own (possibly already
        // running) process, so the PATH is set inside the new tab's shell.
        if let Some(wt) = path_env::which("wt") {
            return Command::new(wt)
                .arg("-d")
                .arg(dir)
                .args(["cmd.exe", "/k", &set_path])
                .spawn()
                .map(|_| ());
        }
        Command::new("cmd.exe")
            .args(["/k", &set_path])
            .current_dir(dir)
            .creation_flags(CREATE_NEW_CONSOLE)
            .spawn()
            .map(|_| ())
    }
    #[cfg(target_os = "macos")]
    {
        // Terminal.app starts a login shell, which reads the PATH block.
        let _ = path_prefix;
        Command::new("open")
            .args(["-a", "Terminal"])
            .arg(dir)
            .spawn()
            .map(|_| ())
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let terminal = std::env::var("TERMINAL")
            .ok()
            .and_then(|t| path_env::which(&t))
            .or_else(|| {
                [
                    "x-terminal-emulator",
                    "gnome-terminal",
                    "konsole",
                    "xfce4-terminal",
                    "xterm",
                ]
                .iter()
                .find_map(|t| path_env::which(t))
            })
            .ok_or_else(|| std::io::Error::other("no terminal emulator found (set $TERMINAL)"))?;
        spawn_detached(&terminal, &[], Some(dir), Some(path_prefix))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_arguments() {
        let args = vec!["{path}".to_owned()];
        let line = vec!["--goto".to_owned(), "{path}:{line}".to_owned()];
        let p = Path::new("/www/blog/app.php");
        assert_eq!(editor_args(&args, &line, p, None), ["/www/blog/app.php"]);
        assert_eq!(
            editor_args(&args, &line, p, Some(12)),
            ["--goto", "/www/blog/app.php:12"]
        );
        // No line template: the plain one is used.
        assert_eq!(editor_args(&args, &[], p, Some(12)), ["/www/blog/app.php"]);
        // A template without {path} still gets the path.
        assert_eq!(
            editor_args(&["-n".to_owned()], &[], p, None),
            ["-n", "/www/blog/app.php"]
        );
        let npp = vec!["-n{line}".to_owned(), "{path}".to_owned()];
        assert_eq!(
            editor_args(&args, &npp, p, Some(3)),
            ["-n3", "/www/blog/app.php"]
        );
    }

    #[test]
    fn variables_expand_or_fail() {
        let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
        let value = std::env::var(var).expect("home");
        assert_eq!(
            expand_vars(&format!("%{var}%/x")),
            Some(PathBuf::from(format!("{value}/x")))
        );
        assert_eq!(expand_vars("%NO_SUCH_VAR_42%/x"), None);
    }

    #[test]
    fn path_prefix_comes_first_once() {
        let prefix = std::env::temp_dir().join("lumina-shims-test");
        let joined = prefixed_path(&prefix);
        let dirs: Vec<PathBuf> = std::env::split_paths(&joined).collect();
        assert_eq!(dirs.first(), Some(&prefix));
        assert_eq!(dirs.iter().filter(|d| **d == prefix).count(), 1);
    }
}

#[cfg(test)]
mod by_hand {
    /// Opens LUMINA_EDITOR_TEST_PATH with every detected editor, as "Open in
    /// editor" does. Opens real windows:
    /// `cargo test -p lumina-platform open_with_every_preset -- --ignored --nocapture`
    #[test]
    #[ignore = "opens editor windows"]
    fn open_with_every_preset() {
        let path = std::path::PathBuf::from(
            std::env::var("LUMINA_EDITOR_TEST_PATH").expect("set LUMINA_EDITOR_TEST_PATH"),
        );
        for p in super::editor_presets() {
            let args = super::editor_args(&p.args, &p.args_line, &path, None);
            super::spawn_detached(&p.command, &args, Some(&path), None).expect("spawn");
            println!("{}: {} {:?}", p.id, p.command.display(), args);
        }
    }
}
