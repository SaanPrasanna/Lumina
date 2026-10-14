//! Run privileged operations: UAC (runas) on Windows, authorization prompt on macOS, pkexec on Linux.
//!
//! The GUI never runs elevated. It runs `lumina-helper` with one validated
//! sub-command through [`run_elevated`], which shows the OS prompt once and
//! waits for the helper to exit. Blocking: call it from a blocking thread.

use std::ffi::OsStr;
use std::fmt;
use std::path::Path;
use std::time::Duration;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

/// How long the elevated program may run once the user has approved it.
pub const RUN_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElevationError {
    /// The user dismissed the prompt or was not authorised.
    Declined,
    /// The OS has no way to elevate here (e.g. no pkexec), or launching failed.
    Unavailable(String),
    /// The program was started but did not finish within [`RUN_TIMEOUT`].
    TimedOut,
}

impl fmt::Display for ElevationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Declined => f.write_str("the administrator prompt was declined"),
            Self::Unavailable(why) => write!(f, "cannot run with administrator rights: {why}"),
            Self::TimedOut => f.write_str("the elevated helper did not finish in time"),
        }
    }
}

impl std::error::Error for ElevationError {}

/// What the elevated program reported. `stderr` is empty on Windows, where an
/// elevated process's output cannot be captured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElevatedExit {
    pub code: i32,
    pub stderr: String,
}

/// Runs `program args...` with administrator/root rights after one OS prompt
/// and returns its exit code. `program` must be an absolute path.
pub fn run_elevated(program: &Path, args: &[&OsStr]) -> Result<ElevatedExit, ElevationError> {
    if !program.is_absolute() {
        return Err(ElevationError::Unavailable(format!(
            "{} is not an absolute path",
            program.display()
        )));
    }
    #[cfg(target_os = "windows")]
    return windows::run(program, args);
    #[cfg(target_os = "macos")]
    return macos::run(program, args);
    #[cfg(target_os = "linux")]
    return linux::run(program, args);
    #[allow(unreachable_code)]
    Err(ElevationError::Unavailable("unsupported platform".into()))
}
