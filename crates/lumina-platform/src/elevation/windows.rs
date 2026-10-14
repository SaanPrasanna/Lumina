//! elevation: windows implementation (`ShellExecuteExW` with the `runas` verb,
//! which shows the UAC prompt, then wait for the process).
#![allow(unsafe_code)]

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_CANCELLED, GetLastError, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows_sys::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};
use windows_sys::Win32::UI::Shell::{
    SEE_MASK_FLAG_NO_UI, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;

use super::{ElevatedExit, ElevationError, RUN_TIMEOUT};

fn wide(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(std::iter::once(0)).collect()
}

/// Quotes one argument so `CommandLineToArgvW` (and the Rust runtime) reads it
/// back unchanged: backslashes are literal except before a quote.
pub(super) fn quote_arg(arg: &str) -> String {
    if !arg.is_empty() && !arg.contains([' ', '\t', '\n', '\u{b}', '"']) {
        return arg.to_owned();
    }
    let mut out = String::from("\"");
    let mut backslashes = 0;
    for c in arg.chars() {
        match c {
            '\\' => backslashes += 1,
            '"' => {
                out.extend(std::iter::repeat_n('\\', backslashes * 2 + 1));
                out.push('"');
                backslashes = 0;
            }
            _ => {
                out.extend(std::iter::repeat_n('\\', backslashes));
                out.push(c);
                backslashes = 0;
            }
        }
    }
    out.extend(std::iter::repeat_n('\\', backslashes * 2));
    out.push('"');
    out
}

pub(super) fn run(program: &Path, args: &[&OsStr]) -> Result<ElevatedExit, ElevationError> {
    let params = args
        .iter()
        .map(|a| quote_arg(&a.to_string_lossy()))
        .collect::<Vec<_>>()
        .join(" ");
    let verb = wide(OsStr::new("runas"));
    let file = wide(program.as_os_str());
    let params = wide(OsStr::new(&params));
    let dir = program.parent().map(|d| wide(d.as_os_str()));

    // SAFETY: zero is a valid bit pattern for this plain C struct.
    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_FLAG_NO_UI;
    info.lpVerb = verb.as_ptr();
    info.lpFile = file.as_ptr();
    info.lpParameters = params.as_ptr();
    info.lpDirectory = dir.as_ref().map_or(std::ptr::null(), |d| d.as_ptr());
    info.nShow = SW_HIDE;

    // SAFETY: every pointer in `info` refers to a NUL-terminated buffer that
    // outlives the call.
    let ok = unsafe { ShellExecuteExW(&mut info) };
    if ok == 0 {
        // SAFETY: no other call in between.
        let err = unsafe { GetLastError() };
        if err == ERROR_CANCELLED {
            return Err(ElevationError::Declined);
        }
        return Err(ElevationError::Unavailable(format!(
            "ShellExecuteEx failed: {}",
            std::io::Error::from_raw_os_error(err as i32)
        )));
    }
    let process = info.hProcess;
    if process.is_null() {
        return Err(ElevationError::Unavailable("no process handle".into()));
    }

    let timeout_ms = u32::try_from(RUN_TIMEOUT.as_millis()).unwrap_or(u32::MAX);
    // SAFETY: `process` is a valid handle we own until CloseHandle below.
    let waited = unsafe { WaitForSingleObject(process, timeout_ms) };
    let result = match waited {
        WAIT_OBJECT_0 => {
            let mut code = 0u32;
            // SAFETY: as above; `code` is a valid out pointer.
            if unsafe { GetExitCodeProcess(process, &mut code) } == 0 {
                Err(ElevationError::Unavailable(
                    "could not read the exit code".into(),
                ))
            } else {
                Ok(ElevatedExit {
                    code: code as i32,
                    stderr: String::new(),
                })
            }
        }
        WAIT_TIMEOUT => Err(ElevationError::TimedOut),
        _ => Err(ElevationError::Unavailable(
            "waiting for the helper failed".into(),
        )),
    };
    // SAFETY: closing the handle we were given.
    unsafe { CloseHandle(process) };
    result
}

#[cfg(test)]
mod tests {
    use super::quote_arg;

    #[test]
    fn quoting_round_trips_through_the_windows_rules() {
        assert_eq!(quote_arg("hosts-apply"), "hosts-apply");
        assert_eq!(quote_arg(r"C:\Lumina\tmp\a.json"), r"C:\Lumina\tmp\a.json");
        assert_eq!(
            quote_arg(r"C:\My Data\tmp\a.json"),
            r#""C:\My Data\tmp\a.json""#
        );
        assert_eq!(quote_arg(r"C:\dir with space\"), r#""C:\dir with space\\""#);
        assert_eq!(quote_arg(r#"a"b"#), r#""a\"b""#);
        assert_eq!(quote_arg(""), r#""""#);
    }
}
