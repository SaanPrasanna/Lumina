//! path_env: macOS and Linux implementation (shell start-up files).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use super::{PathStatus, add_block, remove_block, shadows};

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn shell_name() -> String {
    std::env::var("SHELL")
        .ok()
        .and_then(|s| s.rsplit('/').next().map(str::to_owned))
        .unwrap_or_default()
}

/// The start-up files to edit: those that exist, plus the one the user's
/// shell reads if it is missing.
fn rc_files(home: &Path) -> Vec<PathBuf> {
    let shell = shell_name();
    let mut files: Vec<PathBuf> = [".zshrc", ".bashrc", ".bash_profile", ".profile"]
        .iter()
        .map(|f| home.join(f))
        .filter(|p| p.is_file())
        .collect();
    let wanted = match shell.as_str() {
        "zsh" => Some(".zshrc"),
        "bash" if cfg!(target_os = "macos") => Some(".bash_profile"),
        "bash" => Some(".bashrc"),
        _ => None,
    };
    if let Some(name) = wanted {
        let path = home.join(name);
        if !files.contains(&path) {
            files.push(path);
        }
    }
    if files.is_empty() {
        files.push(home.join(".profile"));
    }
    files
}

fn fish_file(home: &Path) -> PathBuf {
    home.join(".config/fish/conf.d/lumina.fish")
}

fn body(dir: &Path) -> String {
    format!("export PATH=\"{}:$PATH\"", dir.display())
}

pub(super) fn add(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let Some(home) = home() else {
        return Err(std::io::Error::other("HOME is not set"));
    };
    let mut changed = Vec::new();
    for file in rc_files(&home) {
        let text = std::fs::read_to_string(&file).unwrap_or_default();
        let next = add_block(&text, &body(dir));
        if next != text {
            std::fs::write(&file, next)?;
            changed.push(file);
        }
    }
    if shell_name() == "fish" || home.join(".config/fish").is_dir() {
        let file = fish_file(&home);
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(
            &file,
            format!(
                "# Added by Lumina; removed by \"Remove from PATH\".\nfish_add_path --prepend \"{}\"\n",
                dir.display()
            ),
        )?;
        changed.push(file);
    }
    Ok(changed)
}

pub(super) fn remove(_dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let Some(home) = home() else {
        return Ok(Vec::new());
    };
    let mut changed = Vec::new();
    for name in [".zshrc", ".bashrc", ".bash_profile", ".profile"] {
        let file = home.join(name);
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        let next = remove_block(&text);
        if next != text {
            std::fs::write(&file, next)?;
            changed.push(file);
        }
    }
    let fish = fish_file(&home);
    if fish.exists() {
        std::fs::remove_file(&fish)?;
        changed.push(fish);
    }
    Ok(changed)
}

/// PATH as a new interactive login shell sees it.
fn shell_path() -> Option<String> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
    let mut child = Command::new(shell)
        .args(["-lic", "printf %s \"$PATH\""])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        if child.try_wait().ok()?.is_some() {
            break;
        }
        if std::time::Instant::now() > deadline {
            let _ = child.kill();
            return None;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let out = child.wait_with_output().ok()?;
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub(super) fn status(dir: &Path) -> PathStatus {
    let files: Vec<PathBuf> = home()
        .map(|h| {
            let mut found: Vec<PathBuf> = [".zshrc", ".bashrc", ".bash_profile", ".profile"]
                .iter()
                .map(|f| h.join(f))
                .filter(|p| {
                    std::fs::read_to_string(p).is_ok_and(|t| t.contains(super::BLOCK_BEGIN))
                })
                .collect();
            if fish_file(&h).exists() {
                found.push(fish_file(&h));
            }
            found
        })
        .unwrap_or_default();
    let dirs: Vec<PathBuf> = shell_path()
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    PathStatus {
        added: !files.is_empty(),
        added_system: false,
        files,
        shadowed: shadows(&dirs, dir),
    }
}
