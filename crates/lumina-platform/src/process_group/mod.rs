//! Kill-on-close process grouping. Every service Lumina starts lives in its own
//! group, so stopping it, switching versions, or Lumina itself dying takes the
//! whole tree down — no ghost `php-cgi` processes.
//!
//! - **Windows**: a Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`. The child
//!   is spawned suspended, assigned to the job, then resumed, so it cannot start a
//!   grandchild outside the job. The OS kills the job when its last handle closes,
//!   which includes Lumina crashing.
//! - **Unix**: the child leads a new process group (`setpgid(0, 0)`); stopping
//!   signals the whole group. A crash of Lumina is covered by the PID ledger sweep
//!   in `lumina-process`, because Unix has no kill-on-parent-exit for a whole tree.
//!
//! Usage: [`configure`] the command, spawn it, then [`ProcessGroup::attach`].

use std::io;
use std::process::Command;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
use unix as imp;
#[cfg(windows)]
use windows as imp;

/// A just-spawned child, as needed by [`ProcessGroup::attach`].
pub struct SpawnedChild {
    pub pid: u32,
    /// The process handle (e.g. `tokio::process::Child::raw_handle`).
    #[cfg(windows)]
    pub handle: std::os::windows::io::RawHandle,
}

/// Prepares `cmd` to be spawned into a new group. Must be called before spawning.
pub fn configure(cmd: &mut Command) {
    imp::configure(cmd);
}

/// A group of processes that is terminated as a unit.
///
/// Dropping the group kills every process still in it.
pub struct ProcessGroup {
    inner: imp::Group,
}

impl ProcessGroup {
    /// Puts a child spawned with a [`configure`]d command into a new group and lets
    /// it run. If this fails, the caller must kill the child (on Windows it is
    /// still suspended).
    pub fn attach(child: &SpawnedChild) -> io::Result<Self> {
        Ok(Self {
            inner: imp::Group::attach(child)?,
        })
    }

    /// PID of the process the group was created for.
    pub fn leader_pid(&self) -> u32 {
        self.inner.leader_pid()
    }

    /// Asks every process to exit (SIGTERM on Unix). Windows has no graceful
    /// equivalent for console-less servers, so this is the same as [`Self::kill`].
    pub fn terminate(&self) -> io::Result<()> {
        self.inner.terminate()
    }

    /// Forcefully kills every process in the group.
    pub fn kill(&self) -> io::Result<()> {
        self.inner.kill()
    }

    /// PIDs of the live processes in the group.
    pub fn members(&self) -> io::Result<Vec<u32>> {
        self.inner.members()
    }

    /// `true` once no live process remains in the group.
    pub fn is_empty(&self) -> io::Result<bool> {
        Ok(self.members()?.is_empty())
    }
}

/// Kills a group that is no longer owned by a [`ProcessGroup`] (e.g. one recorded
/// in the PID ledger by a Lumina instance that crashed). On Unix this signals the
/// process group led by `leader_pid`. On Windows the job died with its owner, so
/// only the leader process itself can still be alive; it is killed by PID.
pub fn kill_orphaned_group(leader_pid: u32) -> io::Result<()> {
    imp::kill_orphaned_group(leader_pid)
}

/// Candidate PIDs belonging to an orphaned group led by `leader_pid`: every live
/// member on Unix (the leader may already be gone while workers live on), or just
/// the leader on Windows. Callers must verify each PID before killing anything,
/// because PIDs are reused.
pub fn orphaned_members(leader_pid: u32) -> io::Result<Vec<u32>> {
    imp::orphaned_members(leader_pid)
}
