//! Developer automation: `cargo xtask <task>`.
//!
//! - `sidecars [--target <triple>]`: builds `lumina-helper` and `lumina-shim`
//!   in release mode and copies them to `apps/desktop/src-tauri/binaries/` as
//!   `<name>-<triple>[.exe]`, where Tauri's `bundle.externalBin` picks them up.
//!   Without `--target`, `TAURI_ENV_TARGET_TRIPLE` (set by `tauri build`) or
//!   the host triple is used; `universal-apple-darwin` builds both macOS
//!   architectures and joins them with `lipo`.

use std::path::{Path, PathBuf};
use std::process::Command;

const SIDECARS: [&str; 2] = ["lumina-helper", "lumina-shim"];

type Result<T> = std::result::Result<T, String>;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("sidecars") => sidecars(&args[1..]),
        Some(other) => Err(format!("unknown task: {other}")),
        None => Err("usage: cargo xtask sidecars [--target <triple>]".into()),
    };
    if let Err(err) = result {
        eprintln!("xtask: {err}");
        std::process::exit(1);
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn host_triple() -> Result<String> {
    let out = Command::new("rustc")
        .arg("-vV")
        .output()
        .map_err(|e| format!("rustc -vV: {e}"))?;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| l.strip_prefix("host: ").map(str::to_owned))
        .ok_or_else(|| "rustc -vV printed no host triple".into())
}

fn run(cmd: &mut Command) -> Result<()> {
    eprintln!("xtask: {cmd:?}");
    let status = cmd.status().map_err(|e| format!("{cmd:?}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{cmd:?} failed: {status}"))
    }
}

/// Builds the sidecars for `triple` and returns where each binary is.
fn build(root: &Path, triple: &str) -> Result<Vec<PathBuf>> {
    let mut cmd = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    cmd.current_dir(root)
        .args(["build", "--release", "--locked", "--target", triple]);
    for name in SIDECARS {
        cmd.args(["-p", name]);
    }
    run(&mut cmd)?;
    let exe = if triple.contains("windows") {
        ".exe"
    } else {
        ""
    };
    Ok(SIDECARS
        .iter()
        .map(|name| {
            root.join("target")
                .join(triple)
                .join("release")
                .join(format!("{name}{exe}"))
        })
        .collect())
}

fn sidecars(args: &[String]) -> Result<()> {
    let triple = match args {
        [flag, triple] if flag == "--target" => triple.clone(),
        [] => match std::env::var("TAURI_ENV_TARGET_TRIPLE") {
            Ok(t) if !t.is_empty() => t,
            _ => host_triple()?,
        },
        _ => return Err("usage: cargo xtask sidecars [--target <triple>]".into()),
    };
    let root = workspace_root();
    let out = root.join("apps/desktop/src-tauri/binaries");
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;

    if triple == "universal-apple-darwin" {
        let arm = build(&root, "aarch64-apple-darwin")?;
        let intel = build(&root, "x86_64-apple-darwin")?;
        for ((name, a), i) in SIDECARS.iter().zip(arm).zip(intel) {
            let dest = out.join(format!("{name}-{triple}"));
            run(Command::new("lipo")
                .arg("-create")
                .arg(&a)
                .arg(&i)
                .arg("-output")
                .arg(&dest))?;
            eprintln!("xtask: {}", dest.display());
        }
        return Ok(());
    }

    let built = build(&root, &triple)?;
    let exe = if triple.contains("windows") {
        ".exe"
    } else {
        ""
    };
    for (name, from) in SIDECARS.iter().zip(built) {
        let dest = out.join(format!("{name}-{triple}{exe}"));
        std::fs::copy(&from, &dest)
            .map_err(|e| format!("{} -> {}: {e}", from.display(), dest.display()))?;
        eprintln!("xtask: {}", dest.display());
    }
    Ok(())
}
