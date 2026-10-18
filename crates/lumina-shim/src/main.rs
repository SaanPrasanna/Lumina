//! `php`, `composer`, `node`, ... in Lumina's PATH folder: runs the program
//! named in `<name>.shim`, passes the arguments through and exits with its
//! exit code. Ctrl+C reaches the program (it shares the console); the shim
//! itself ignores it and waits, so there is no "Terminate batch job?" prompt.
//!
//! When Lumina provides no such program (the `.shim` file or its target is
//! gone), the next `<name>` on PATH runs instead, as if the shim were not there.

use std::ffi::OsString;
use std::path::Path;
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
    let target = std::fs::read_to_string(&file)
        .ok()
        .and_then(|text| lumina_shim::parse(&text));
    match target {
        Some(target) if target.program.is_file() => run(&target.program, &target.args, false),
        target => {
            // Not provided by Lumina: the next program of that name on PATH.
            if std::env::var_os(lumina_shim::FALLBACK_ENV).is_none()
                && let Some(next) = exe.parent().and_then(|shim_dir| {
                    let path = std::env::var_os("PATH").unwrap_or_default();
                    lumina_shim::find_on_path(&name, &path, shim_dir)
                })
            {
                return run(&next, &[], true);
            }
            match target {
                Some(target) => eprintln!(
                    "lumina: `{name}` points to {}, which no longer exists. Open Lumina to refresh it.",
                    target.program.display()
                ),
                None => eprintln!(
                    "lumina: `{name}` is not set up ({} is missing). Open Lumina and install it in Quick Add.",
                    file.display()
                ),
            }
            ExitCode::from(127)
        }
    }
}

/// Runs `program` with `args` and the shim's own arguments; returns its code.
fn run(program: &Path, args: &[String], fallback: bool) -> ExitCode {
    ignore_ctrl_c();
    // std quotes arguments correctly for .exe and for .cmd/.bat targets (npm.cmd).
    let mut command = Command::new(program);
    command.args(args).args(std::env::args_os().skip(1));
    if fallback {
        command.env(lumina_shim::FALLBACK_ENV, OsString::from("1"));
    }
    let status = command.spawn().and_then(|mut child| {
        let job = end_with_shim(&child);
        let status = child.wait();
        let_live(job);
        status
    });
    match status {
        // The full code (Windows codes are 32-bit, e.g. 0xC0000005 for a crash).
        Ok(status) => match status.code() {
            Some(code) => std::process::exit(code),
            None => ExitCode::from(130),
        },
        Err(e) => {
            eprintln!("lumina: cannot run {}: {e}", program.display());
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

/// Puts `child` in a job that ends it, and whatever it started, when the
/// shim is killed. Without it, a tool that stops `node` (the shim) by
/// killing it, as `tauri dev` does with Vite, leaves the real node.exe
/// running and holding its port. Returns the job for [`let_live`].
#[cfg(windows)]
#[allow(unsafe_code)]
fn end_with_shim(child: &std::process::Child) -> Option<Job> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    // SAFETY: plain Win32 calls on a job handle created here and the child's
    // live process handle. The job handle is deliberately never closed: the
    // system closes it when the shim exits, which is what ends the child.
    unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() {
            return None;
        }
        let job = Job(job);
        if job.set_limits(JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE)
            && AssignProcessToJobObject(job.0, child.as_raw_handle()) != 0
        {
            Some(job)
        } else {
            None
        }
    }
}

/// The program ended on its own: what it started in the background (a
/// daemon such as pm2) keeps running when the shim exits.
#[cfg(windows)]
fn let_live(job: Option<Job>) {
    if let Some(job) = job {
        job.set_limits(0);
    }
}

#[cfg(windows)]
struct Job(windows_sys::Win32::Foundation::HANDLE);

#[cfg(windows)]
impl Job {
    #[allow(unsafe_code)]
    fn set_limits(&self, flags: u32) -> bool {
        use windows_sys::Win32::System::JobObjects::{
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            SetInformationJobObject,
        };
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = flags;
        let size = u32::try_from(std::mem::size_of_val(&info)).unwrap_or(u32::MAX);
        // SAFETY: `info` is a valid JOBOBJECT_EXTENDED_LIMIT_INFORMATION of
        // `size` bytes and `self.0` is a job handle this process owns.
        unsafe {
            SetInformationJobObject(
                self.0,
                JobObjectExtendedLimitInformation,
                std::ptr::from_ref(&info).cast(),
                size,
            ) != 0
        }
    }
}

#[cfg(not(windows))]
type Job = ();

#[cfg(not(windows))]
fn end_with_shim(_child: &std::process::Child) -> Option<Job> {
    None
}

#[cfg(not(windows))]
fn let_live(_job: Option<Job>) {}

#[cfg(not(windows))]
fn ignore_ctrl_c() {
    // Unix shims are shell scripts that `exec`; this binary is not used there.
}
