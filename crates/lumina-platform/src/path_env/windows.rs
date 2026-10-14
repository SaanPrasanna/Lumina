//! path_env: windows implementation (`HKCU\Environment\Path`).
#![allow(unsafe_code)]

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, REG_EXPAND_SZ, REG_SZ, RRF_NOEXPAND,
    RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ, RegGetValueW, RegSetKeyValueW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    HWND_BROADCAST, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_SETTINGCHANGE,
};

use super::{PathStatus, contains_entry, prepend_entry, remove_entry, shadows};

const USER_KEY: &str = "Environment";
const MACHINE_KEY: &str = r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment";

fn wide(s: &str) -> Vec<u16> {
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// The raw (unexpanded) value and its type, or `None` if it does not exist.
fn read(root: HKEY, key: &str) -> std::io::Result<Option<(String, u32)>> {
    let (key_w, name) = (wide(key), wide("Path"));
    let flags = RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ | RRF_NOEXPAND;
    let mut kind = 0u32;
    let mut size = 0u32;
    // SAFETY: size query with a null buffer; all pointers are valid.
    let rc = unsafe {
        RegGetValueW(
            root,
            key_w.as_ptr(),
            name.as_ptr(),
            flags,
            &mut kind,
            std::ptr::null_mut(),
            &mut size,
        )
    };
    if rc == 2 {
        return Ok(None); // ERROR_FILE_NOT_FOUND
    }
    if rc != ERROR_SUCCESS {
        return Err(std::io::Error::from_raw_os_error(rc as i32));
    }
    let mut buf = vec![0u16; (size as usize).div_ceil(2) + 1];
    let mut size = (buf.len() * 2) as u32;
    // SAFETY: `buf` holds `size` bytes.
    let rc = unsafe {
        RegGetValueW(
            root,
            key_w.as_ptr(),
            name.as_ptr(),
            flags,
            &mut kind,
            buf.as_mut_ptr().cast(),
            &mut size,
        )
    };
    if rc != ERROR_SUCCESS {
        return Err(std::io::Error::from_raw_os_error(rc as i32));
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Ok(Some((String::from_utf16_lossy(&buf[..len]), kind)))
}

fn write(value: &str, kind: u32) -> std::io::Result<()> {
    write_to(HKEY_CURRENT_USER, USER_KEY, value, kind)
}

fn write_to(root: HKEY, key: &str, value: &str, kind: u32) -> std::io::Result<()> {
    let (key_w, name, data) = (wide(key), wide("Path"), wide(value));
    // SAFETY: `data` is NUL-terminated and its byte length is passed.
    let rc = unsafe {
        RegSetKeyValueW(
            root,
            key_w.as_ptr(),
            name.as_ptr(),
            kind,
            data.as_ptr().cast(),
            (data.len() * 2) as u32,
        )
    };
    if rc != ERROR_SUCCESS {
        return Err(std::io::Error::from_raw_os_error(rc as i32));
    }
    broadcast();
    Ok(())
}

/// Tells Explorer (and so every terminal started afterwards) that the
/// environment changed.
fn broadcast() {
    let env = wide("Environment");
    let mut result = 0usize;
    // SAFETY: `env` outlives the call; a hung window is skipped after 5 s.
    unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            env.as_ptr() as isize,
            SMTO_ABORTIFHUNG,
            5000,
            &mut result,
        )
    };
}

fn user_key_path() -> PathBuf {
    PathBuf::from(r"HKEY_CURRENT_USER\Environment\Path")
}

pub(super) fn add(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let dir = dir.to_string_lossy();
    let (raw, kind) = read(HKEY_CURRENT_USER, USER_KEY)?.unwrap_or((String::new(), REG_EXPAND_SZ));
    let next = prepend_entry(&raw, &dir);
    if next == raw {
        return Ok(Vec::new());
    }
    // Keep REG_EXPAND_SZ (the Windows default) so %USERPROFILE% entries work.
    let kind = if kind == REG_SZ && !next.contains('%') {
        REG_SZ
    } else {
        REG_EXPAND_SZ
    };
    write(&next, kind)?;
    Ok(vec![user_key_path()])
}

pub(super) fn remove(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let Some((raw, kind)) = read(HKEY_CURRENT_USER, USER_KEY)? else {
        return Ok(Vec::new());
    };
    let next = remove_entry(&raw, &dir.to_string_lossy());
    if next == raw {
        return Ok(Vec::new());
    }
    write(&next, kind)?;
    Ok(vec![user_key_path()])
}

pub(super) fn add_system(dir: &Path) -> std::io::Result<bool> {
    let (raw, kind) =
        read(HKEY_LOCAL_MACHINE, MACHINE_KEY)?.unwrap_or((String::new(), REG_EXPAND_SZ));
    let next = prepend_entry(&raw, &dir.to_string_lossy());
    if next == raw {
        return Ok(false);
    }
    write_to(HKEY_LOCAL_MACHINE, MACHINE_KEY, &next, kind)?;
    Ok(true)
}

pub(super) fn remove_system(dir: &Path) -> std::io::Result<bool> {
    let Some((raw, kind)) = read(HKEY_LOCAL_MACHINE, MACHINE_KEY)? else {
        return Ok(false);
    };
    let next = remove_entry(&raw, &dir.to_string_lossy());
    if next == raw {
        return Ok(false);
    }
    write_to(HKEY_LOCAL_MACHINE, MACHINE_KEY, &next, kind)?;
    Ok(true)
}

/// `%VAR%` expansion with this process's environment.
fn expand(raw: &str) -> String {
    let mut out = String::new();
    let mut rest = raw;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('%') {
            Some(end) => {
                let name = &after[..end];
                match std::env::var(name) {
                    Ok(v) => out.push_str(&v),
                    Err(_) => out.push_str(&rest[start..start + end + 2]),
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

pub(super) fn status(dir: &Path) -> PathStatus {
    let user = read(HKEY_CURRENT_USER, USER_KEY)
        .ok()
        .flatten()
        .map(|v| v.0)
        .unwrap_or_default();
    let machine = read(HKEY_LOCAL_MACHINE, MACHINE_KEY)
        .ok()
        .flatten()
        .map(|v| v.0)
        .unwrap_or_default();
    // A new process gets the machine PATH first, then the user PATH.
    let dirs: Vec<PathBuf> = format!("{machine};{user}")
        .split(';')
        .filter(|e| !e.trim().is_empty())
        .map(|e| PathBuf::from(expand(e.trim())))
        .collect();
    PathStatus {
        added: contains_entry(&user, &dir.to_string_lossy()),
        added_system: contains_entry(&machine, &dir.to_string_lossy()),
        files: Vec::new(),
        shadowed: shadows(&dirs, dir),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_known_variables_only() {
        let home = std::env::var("USERPROFILE").expect("USERPROFILE");
        assert_eq!(expand(r"%USERPROFILE%\bin"), format!(r"{home}\bin"));
        assert_eq!(expand("%NO_SUCH_VAR_123%;x"), "%NO_SUCH_VAR_123%;x");
        assert_eq!(expand("50%"), "50%");
    }
}
