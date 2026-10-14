//! `php`, `composer`, `node`, ... in Lumina's PATH folder: runs the program
//! named in `<name>.shim`, passes the arguments through and exits with its
//! exit code. Ctrl+C reaches the program (it shares the console); the shim
//! itself ignores it and waits, so there is no "Terminate batch job?" prompt.

use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => {
            eprintln!("lumina-shim: {e}");
            return ExitCode::from(126);
        }
    };
    let name = lumina_shim::command_name(&exe);
    let file = lumina_shim::shim_file(&exe);
    let Some(target) = std::fs::read_to_string(&file)
        .ok()
        .and_then(|text| lumina_shim::parse(&text))
    else {
        eprintln!(
            "lumina: `{name}` is not set up ({} is missing). Open Lumina and install it in Quick Add.",
            file.display()
        );
        return ExitCode::from(127);
    };
    if !target.program.is_file() {
        eprintln!(
            "lumina: `{name}` points to {}, which no longer exists. Open Lumina to refresh it.",
            target.program.display()
        );
        return ExitCode::from(127);
    }

    ignore_ctrl_c();
    // std quotes arguments correctly for .exe and for .cmd/.bat targets (npm.cmd).
    let status = Command::new(&target.program)
        .args(&target.args)
        .args(std::env::args_os().skip(1))
        .status();
    match status {
        // The full code (Windows codes are 32-bit, e.g. 0xC0000005 for a crash).
        Ok(status) => match status.code() {
            Some(code) => std::process::exit(code),
            None => ExitCode::from(130),
        },
        Err(e) => {
            eprintln!("lumina: cannot run {}: {e}", target.program.display());
            ExitCode::from(126)
        }
    }
}

#[cfg(windows)]
#[allow(unsafe_code)]
fn ignore_ctrl_c() {
    // SAFETY: a null handler with TRUE makes this process ignore Ctrl+C; the
    // child still receives it and decides what to do.
    unsafe { windows_sys::Win32::System::Console::SetConsoleCtrlHandler(None, 1) };
}

#[cfg(not(windows))]
fn ignore_ctrl_c() {
    // Unix shims are shell scripts that `exec`; this binary is not used there.
}
