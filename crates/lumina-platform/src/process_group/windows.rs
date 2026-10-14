//! Windows: one Job Object per group.
#![allow(unsafe_code)]

use std::io;
use std::os::windows::process::CommandExt;
use std::process::Command;
use std::ptr;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectBasicProcessIdList,
    JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
    TerminateJobObject,
};
use windows_sys::Win32::System::Threading::{
    CREATE_NO_WINDOW, CREATE_SUSPENDED, OpenProcess, OpenThread, PROCESS_TERMINATE, ResumeThread,
    THREAD_SUSPEND_RESUME, TerminateProcess,
};

use super::SpawnedChild;

/// Exit code reported by processes killed through the job.
const KILLED_EXIT_CODE: u32 = 1;

pub(super) fn configure(cmd: &mut Command) {
    // Suspended so it can be put in the job before running any code; no console
    // window because Lumina is a GUI app.
    cmd.creation_flags(CREATE_SUSPENDED | CREATE_NO_WINDOW);
}

/// Owned job handle; closing it kills the job (KILL_ON_JOB_CLOSE).
pub(super) struct Group {
    job: HANDLE,
    leader_pid: u32,
}

// SAFETY: a job handle is a kernel object reference usable from any thread.
unsafe impl Send for Group {}
// SAFETY: every Win32 job API used here is thread-safe on the same handle.
unsafe impl Sync for Group {}

impl Group {
    pub(super) fn attach(child: &SpawnedChild) -> io::Result<Self> {
        // SAFETY: null attributes and name create an unnamed job with default security.
        let job = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
        if job.is_null() {
            return Err(io::Error::last_os_error());
        }
        // From here on, dropping `group` closes the job handle.
        let group = Self {
            job,
            leader_pid: child.pid,
        };

        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: `limits` is a valid JOBOBJECT_EXTENDED_LIMIT_INFORMATION and the
        // length passed matches its size.
        let ok = unsafe {
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                (&raw const limits).cast(),
                size_of_val(&limits) as u32,
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }

        // SAFETY: `child.handle` is the live process handle owned by the caller's
        // Child, which outlives this call.
        if unsafe { AssignProcessToJobObject(job, child.handle as HANDLE) } == 0 {
            return Err(io::Error::last_os_error());
        }

        resume_process_threads(child.pid)?;
        Ok(group)
    }

    pub(super) fn leader_pid(&self) -> u32 {
        self.leader_pid
    }

    pub(super) fn terminate(&self) -> io::Result<()> {
        self.kill()
    }

    pub(super) fn kill(&self) -> io::Result<()> {
        // SAFETY: `self.job` is a valid job handle for the lifetime of `self`.
        if unsafe { TerminateJobObject(self.job, KILLED_EXIT_CODE) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    pub(super) fn members(&self) -> io::Result<Vec<u32>> {
        // JOBOBJECT_BASIC_PROCESS_ID_LIST with room for many PIDs.
        #[repr(C)]
        struct PidList {
            assigned: u32,
            listed: u32,
            ids: [usize; 1024],
        }
        let mut list = PidList {
            assigned: 0,
            listed: 0,
            ids: [0; 1024],
        };
        // SAFETY: `list` matches the layout of JOBOBJECT_BASIC_PROCESS_ID_LIST with
        // a larger trailing array, and its full size is passed.
        let ok = unsafe {
            QueryInformationJobObject(
                self.job,
                JobObjectBasicProcessIdList,
                (&raw mut list).cast(),
                size_of_val(&list) as u32,
                ptr::null_mut(),
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        let listed = (list.listed as usize).min(list.ids.len());
        Ok(list.ids[..listed].iter().map(|&pid| pid as u32).collect())
    }
}

impl Drop for Group {
    fn drop(&mut self) {
        // SAFETY: the handle is valid and closed exactly once. KILL_ON_JOB_CLOSE
        // terminates any process still in the job.
        unsafe { CloseHandle(self.job) };
    }
}

/// Resumes every thread of a process created with CREATE_SUSPENDED. A suspended
/// new process has exactly one thread, but resuming all of them is equally correct.
fn resume_process_threads(pid: u32) -> io::Result<()> {
    // SAFETY: plain snapshot creation; the handle is closed below.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }

    let mut resumed = 0;
    let mut result = Ok(());
    let mut entry = THREADENTRY32 {
        dwSize: size_of::<THREADENTRY32>() as u32,
        ..Default::default()
    };
    // SAFETY: `snapshot` is valid and `entry.dwSize` is initialised as required.
    let mut more = unsafe { Thread32First(snapshot, &mut entry) } != 0;
    while more {
        if entry.th32OwnerProcessID == pid {
            // SAFETY: OpenThread returns null on failure, checked below.
            let thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) };
            if thread.is_null() {
                result = Err(io::Error::last_os_error());
                break;
            }
            // SAFETY: `thread` is a valid handle with THREAD_SUSPEND_RESUME access.
            let previous = unsafe { ResumeThread(thread) };
            // SAFETY: closed exactly once.
            unsafe { CloseHandle(thread) };
            if previous == u32::MAX {
                result = Err(io::Error::last_os_error());
                break;
            }
            resumed += 1;
        }
        // SAFETY: as for Thread32First.
        more = unsafe { Thread32Next(snapshot, &mut entry) } != 0;
    }
    // SAFETY: closed exactly once.
    unsafe { CloseHandle(snapshot) };

    result?;
    if resumed == 0 {
        return Err(io::Error::other(format!(
            "no threads found for process {pid}"
        )));
    }
    Ok(())
}

pub(super) fn kill_orphaned_group(leader_pid: u32) -> io::Result<()> {
    // SAFETY: OpenProcess returns null on failure (e.g. already exited), checked below.
    let process = unsafe { OpenProcess(PROCESS_TERMINATE, 0, leader_pid) };
    if process.is_null() {
        return Ok(()); // already gone
    }
    // SAFETY: `process` has PROCESS_TERMINATE access; closed exactly once.
    let ok = unsafe { TerminateProcess(process, KILLED_EXIT_CODE) };
    let err = io::Error::last_os_error();
    // SAFETY: closed exactly once.
    unsafe { CloseHandle(process) };
    if ok == 0 { Err(err) } else { Ok(()) }
}

pub(super) fn orphaned_members(leader_pid: u32) -> io::Result<Vec<u32>> {
    Ok(vec![leader_pid])
}
