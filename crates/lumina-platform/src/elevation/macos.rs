//! elevation: macos implementation (`osascript ... with administrator
//! privileges`, the standard password prompt). A privileged helper registered
//! through `SMAppService` can replace this later.

use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;

use super::{ElevatedExit, ElevationError};

/// Single-quotes `s` for `/bin/sh`.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Escapes `s` for an AppleScript string literal.
fn applescript_string(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', r"\\").replace('"', "\\\""))
}

pub(super) fn script(program: &Path, args: &[&OsStr]) -> String {
    let mut command = shell_quote(&program.to_string_lossy());
    for arg in args {
        command.push(' ');
        command.push_str(&shell_quote(&arg.to_string_lossy()));
    }
    format!(
        "do shell script {} with administrator privileges",
        applescript_string(&command)
    )
}

pub(super) fn run(program: &Path, args: &[&OsStr]) -> Result<ElevatedExit, ElevationError> {
    let out = Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(script(program, args))
        .output()
        .map_err(|e| ElevationError::Unavailable(format!("osascript: {e}")))?;
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_owned();
    if out.status.success() {
        return Ok(ElevatedExit { code: 0, stderr });
    }
    // "execution error: <helper's stderr> (<exit code>)"; -128 = user cancelled.
    let code = stderr
        .rsplit_once('(')
        .and_then(|(_, tail)| tail.trim_end_matches(')').trim().parse::<i32>().ok());
    match code {
        Some(-128) => Err(ElevationError::Declined),
        Some(code) => Ok(ElevatedExit { code, stderr }),
        None => Err(ElevationError::Unavailable(stderr)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_quotes_paths_for_shell_and_applescript() {
        let s = script(
            Path::new("/Applications/Lumina.app/Contents/MacOS/lumina-helper"),
            &[
                OsStr::new("hosts-apply"),
                OsStr::new("/Users/a b/it's \"x\".json"),
            ],
        );
        assert_eq!(
            s,
            r#"do shell script "'/Applications/Lumina.app/Contents/MacOS/lumina-helper' 'hosts-apply' '/Users/a b/it'\\''s \"x\".json'" with administrator privileges"#
        );
    }
}
