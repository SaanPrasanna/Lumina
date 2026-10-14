//! Unix: the child leads a new process group; the group is signalled as a unit.

use std::io;
use std::os::unix::process::CommandExt;
use std::process::Command;

use nix::errno::Errno;
use nix::sys::signal::{Signal, killpg};
use nix::unistd::Pid;

use super::SpawnedChild;

pub(super) fn configure(cmd: &mut Command) {
    // setpgid(0, 0): the child's PID becomes its process group ID.
    cmd.process_group(0);
}

pub(super) struct Group {
    pgid: u32,
}

impl Group {
    pub(super) fn attach(child: &SpawnedChild) -> io::Result<Self> {
        Ok(Self { pgid: child.pid })
    }

    pub(super) fn leader_pid(&self) -> u32 {
        self.pgid
    }

    pub(super) fn terminate(&self) -> io::Result<()> {
        signal_group(self.pgid, Signal::SIGTERM)
    }

    pub(super) fn kill(&self) -> io::Result<()> {
        signal_group(self.pgid, Signal::SIGKILL)
    }

    pub(super) fn members(&self) -> io::Result<Vec<u32>> {
        group_members(self.pgid)
    }
}

impl Drop for Group {
    fn drop(&mut self) {
        let _ = self.kill();
    }
}

fn signal_group(pgid: u32, signal: Signal) -> io::Result<()> {
    let pgid = i32::try_from(pgid).map_err(io::Error::other)?;
    match killpg(Pid::from_raw(pgid), signal) {
        Ok(()) | Err(Errno::ESRCH) => Ok(()), // ESRCH: the group is already empty
        Err(e) => Err(e.into()),
    }
}

pub(super) fn kill_orphaned_group(leader_pid: u32) -> io::Result<()> {
    signal_group(leader_pid, Signal::SIGKILL)
}

/// Live (non-zombie) processes whose process group is `pgid`, from /proc.
#[cfg(target_os = "linux")]
fn group_members(pgid: u32) -> io::Result<Vec<u32>> {
    let mut members = Vec::new();
    for entry in std::fs::read_dir("/proc")? {
        let Ok(entry) = entry else { continue };
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        // The process may exit between read_dir and read; skip it then.
        let Ok(stat) = std::fs::read_to_string(entry.path().join("stat")) else {
            continue;
        };
        // Format: "pid (comm) state ppid pgrp ...". `comm` may contain spaces and
        // parentheses, so split after the last ')'.
        let Some(rest) = stat.rfind(')').map(|i| &stat[i + 1..]) else {
            continue;
        };
        let mut fields = rest.split_whitespace();
        let state = fields.next();
        let pgrp = fields.nth(1).and_then(|s| s.parse::<u32>().ok());
        if pgrp == Some(pgid) && state != Some("Z") {
            members.push(pid);
        }
    }
    Ok(members)
}

/// Live (non-zombie) processes whose process group is `pgid`, from `ps`.
#[cfg(not(target_os = "linux"))]
fn group_members(pgid: u32) -> io::Result<Vec<u32>> {
    let output = Command::new("ps")
        .args(["-A", "-o", "pid=,pgid=,stat="])
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other("ps failed"));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let pid = fields.next()?.parse::<u32>().ok()?;
            let group = fields.next()?.parse::<u32>().ok()?;
            let stat = fields.next().unwrap_or("");
            (group == pgid && !stat.starts_with('Z')).then_some(pid)
        })
        .collect())
}

pub(super) fn orphaned_members(leader_pid: u32) -> io::Result<Vec<u32>> {
    group_members(leader_pid)
}
