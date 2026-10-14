//! elevation: linux implementation (`pkexec`, which shows the desktop's polkit
//! prompt; packaging/linux ships a policy with a readable message).

use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;

use super::{ElevatedExit, ElevationError};

/// pkexec: dismissed or not authorised.
const PKEXEC_NOT_AUTHORIZED: i32 = 126;
/// pkexec: authentication failed or no authentication agent.
const PKEXEC_AUTH_FAILED: i32 = 127;

pub(super) fn run(program: &Path, args: &[&OsStr]) -> Result<ElevatedExit, ElevationError> {
    let out = Command::new("pkexec")
        .arg(program)
        .args(args)
        .output()
        .map_err(|e| ElevationError::Unavailable(format!("pkexec: {e}")))?;
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_owned();
    match out.status.code() {
        // The helper's own exit codes are all below 64, so these are pkexec's.
        Some(PKEXEC_NOT_AUTHORIZED | PKEXEC_AUTH_FAILED) => Err(ElevationError::Declined),
        Some(code) => Ok(ElevatedExit { code, stderr }),
        None => Err(ElevationError::Unavailable("pkexec was killed".into())),
    }
}
