//! Owns every child process: starts each in its own kill-on-close group, restarts
//! on crash per [`RestartPolicy`], and stops with **verification** that no process
//! of the group survives. That verification is what prevents ghost processes.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex, MutexGuard};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use lumina_core::{Error, Result};
use lumina_platform::process_group::{self, ProcessGroup, SpawnedChild};
use tokio::process::Child;
use tokio::sync::{Mutex, Notify, broadcast};
use tokio::task::JoinHandle;
use tokio::time::{Instant, sleep, timeout};

use crate::handle::{ProcessEvent, ProcessInfo, ProcessSpec, ProcessState, RestartPolicy};
use crate::ledger::Ledger;
use crate::logs;

/// How often `stop` re-checks that a group is empty.
const VERIFY_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Clone)]
pub struct Supervisor {
    inner: Arc<Inner>,
}

struct Inner {
    procs: Mutex<HashMap<String, Arc<Managed>>>,
    ledger: Ledger,
    events: broadcast::Sender<ProcessEvent>,
}

struct Managed {
    spec: ProcessSpec,
    stopping: AtomicBool,
    /// Wakes the monitor out of a restart backoff when `stop` is called.
    stop_signal: Notify,
    run: StdMutex<Run>,
    monitor: StdMutex<Option<JoinHandle<()>>>,
}

/// The current generation of a managed process (replaced on each restart).
struct Run {
    pid: u32,
    group: Arc<ProcessGroup>,
    started_at: u64,
    restarts: u32,
    state: ProcessState,
}

impl Managed {
    fn run(&self) -> MutexGuard<'_, Run> {
        self.run.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn group(&self) -> Arc<ProcessGroup> {
        Arc::clone(&self.run().group)
    }

    fn info(&self) -> ProcessInfo {
        let run = self.run();
        ProcessInfo {
            id: self.spec.id.clone(),
            pid: run.pid,
            started_at: run.started_at,
            restarts: run.restarts,
            state: run.state,
        }
    }
}

impl Supervisor {
    pub fn new(ledger: Ledger) -> Self {
        let (events, _) = broadcast::channel(64);
        Self {
            inner: Arc::new(Inner {
                procs: Mutex::new(HashMap::new()),
                ledger,
                events,
            }),
        }
    }

    pub fn ledger(&self) -> &Ledger {
        &self.inner.ledger
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ProcessEvent> {
        self.inner.events.subscribe()
    }

    /// Starts `spec`. Fails if a process with the same ID is running or restarting.
    pub async fn start(&self, spec: ProcessSpec) -> Result<ProcessInfo> {
        let mut procs = self.inner.procs.lock().await;
        if let Some(existing) = procs.get(&spec.id)
            && existing.run().state != ProcessState::Failed
        {
            return Err(Error::Service(spec.id.clone(), "already running".into()));
        }

        let (child, group) = spawn(&spec, &self.inner.ledger)?;
        let pid = group.leader_pid();
        let managed = Arc::new(Managed {
            spec,
            stopping: AtomicBool::new(false),
            stop_signal: Notify::new(),
            run: StdMutex::new(Run {
                pid,
                group: Arc::new(group),
                started_at: unix_now(),
                restarts: 0,
                state: ProcessState::Running,
            }),
            monitor: StdMutex::new(None),
        });
        let task = tokio::spawn(monitor(
            Arc::clone(&self.inner),
            Arc::clone(&managed),
            child,
        ));
        *managed.monitor.lock().unwrap_or_else(|e| e.into_inner()) = Some(task);

        let id = managed.spec.id.clone();
        tracing::info!("started `{id}` (pid {pid})");
        let _ = self.inner.events.send(ProcessEvent::Started {
            id: id.clone(),
            pid,
        });
        let info = managed.info();
        procs.insert(id, managed);
        Ok(info)
    }

    /// Stops `id` and verifies that every process in its group is gone.
    ///
    /// The first half of `limit` is a grace period after asking processes to exit
    /// (SIGTERM on Unix); after that they are killed. An error means processes
    /// survived even the kill, which should never happen.
    pub async fn stop(&self, id: &str, limit: Duration) -> Result<()> {
        self.stop_inner(id, limit, None).await
    }

    /// Like [`Self::stop`], but first runs `graceful` (e.g. `nginx -s quit`) and
    /// gives the server half of `limit` to exit on its own before it is
    /// terminated. The process is marked as stopping *before* `graceful` runs, so
    /// its own exit is not mistaken for a crash and restarted.
    pub async fn stop_gracefully<F>(&self, id: &str, limit: Duration, graceful: F) -> Result<()>
    where
        F: Future<Output = ()> + Send,
    {
        self.stop_inner(id, limit, Some(Box::pin(graceful))).await
    }

    async fn stop_inner(
        &self,
        id: &str,
        limit: Duration,
        graceful: Option<Pin<Box<dyn Future<Output = ()> + Send + '_>>>,
    ) -> Result<()> {
        let Some(managed) = self.inner.procs.lock().await.get(id).cloned() else {
            return Ok(()); // not running
        };
        managed.stopping.store(true, Ordering::SeqCst);
        managed.stop_signal.notify_one();

        let mut task = managed
            .monitor
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        let mut exited = false;
        if let Some(graceful) = graceful {
            graceful.await;
            if let Some(task) = task.as_mut() {
                exited = timeout(limit / 2, task).await.is_ok();
            }
        }
        if !exited {
            let _ = managed.group().terminate();
            if let Some(mut task) = task
                && timeout(limit / 2, &mut task).await.is_err()
            {
                // Re-read the group: a restart may have replaced it meanwhile.
                let _ = managed.group().kill();
                let _ = timeout(limit / 2, &mut task).await;
            }
        }

        let group = managed.group();
        let deadline = Instant::now() + limit;
        loop {
            match group.members() {
                Ok(members) if members.is_empty() => break,
                Ok(members) if Instant::now() >= deadline => {
                    return Err(Error::Service(
                        id.to_owned(),
                        format!("processes still running after stop: {members:?}"),
                    ));
                }
                Ok(_) => {
                    let _ = group.kill();
                }
                Err(err) => {
                    tracing::warn!("cannot verify that `{id}` stopped: {err}");
                    break;
                }
            }
            sleep(VERIFY_INTERVAL).await;
        }

        let pid = managed.run().pid;
        self.inner.ledger.remove(pid);
        self.inner.procs.lock().await.remove(id);
        tracing::info!("stopped `{id}` (pid {pid})");
        let _ = self.inner.events.send(ProcessEvent::Exited {
            id: id.to_owned(),
            code: None,
            expected: true,
        });
        Ok(())
    }

    /// Stops everything (app exit). Returns the processes that failed to stop.
    pub async fn stop_all(&self, limit: Duration) -> Vec<(String, Error)> {
        let ids: Vec<String> = self.inner.procs.lock().await.keys().cloned().collect();
        let mut failures = Vec::new();
        for id in ids {
            if let Err(err) = self.stop(&id, limit).await {
                tracing::error!("{err}");
                failures.push((id, err));
            }
        }
        failures
    }

    pub async fn info(&self, id: &str) -> Option<ProcessInfo> {
        self.inner.procs.lock().await.get(id).map(|m| m.info())
    }

    /// PIDs of every live process in `id`'s group (the leader and its workers).
    pub async fn members(&self, id: &str) -> Vec<u32> {
        let managed = self.inner.procs.lock().await.get(id).cloned();
        managed
            .and_then(|m| m.group().members().ok())
            .unwrap_or_default()
    }
}

/// Waits for the process to exit and applies the restart policy.
async fn monitor(inner: Arc<Inner>, managed: Arc<Managed>, mut child: Child) {
    let id = managed.spec.id.clone();
    loop {
        let code = child.wait().await.ok().and_then(|s| s.code());
        // Workers must not outlive their leader.
        let _ = managed.group().kill();
        if managed.stopping.load(Ordering::SeqCst) {
            return; // `stop` finishes the bookkeeping
        }

        let (pid, restarts) = {
            let run = managed.run();
            (run.pid, run.restarts)
        };
        inner.ledger.remove(pid);
        tracing::warn!("`{id}` (pid {pid}) exited unexpectedly with code {code:?}");
        let _ = inner.events.send(ProcessEvent::Exited {
            id: id.clone(),
            code,
            expected: false,
        });

        let RestartPolicy::OnFailure {
            max_restarts,
            backoff,
        } = managed.spec.restart
        else {
            return fail(&inner, &managed, format!("exited with code {code:?}"));
        };
        if restarts >= max_restarts {
            return fail(
                &inner,
                &managed,
                format!("exited with code {code:?}; gave up after {restarts} restarts"),
            );
        }

        let attempt = restarts + 1;
        managed.run().state = ProcessState::Restarting;
        let _ = inner.events.send(ProcessEvent::Restarting {
            id: id.clone(),
            attempt,
        });
        tokio::select! {
            () = sleep(backoff * attempt) => {}
            () = managed.stop_signal.notified() => return,
        }

        match spawn(&managed.spec, &inner.ledger) {
            Ok((next_child, group)) => {
                let pid = group.leader_pid();
                child = next_child;
                {
                    let mut run = managed.run();
                    run.pid = pid;
                    run.group = Arc::new(group);
                    run.started_at = unix_now();
                    run.restarts = attempt;
                    run.state = ProcessState::Running;
                }
                if managed.stopping.load(Ordering::SeqCst) {
                    // `stop` raced with the respawn; don't leave the new one running.
                    let _ = managed.group().kill();
                }
                tracing::info!("restarted `{id}` (pid {pid}, attempt {attempt})");
                let _ = inner.events.send(ProcessEvent::Started {
                    id: id.clone(),
                    pid,
                });
            }
            Err(err) => return fail(&inner, &managed, format!("restart failed: {err}")),
        }
    }
}

fn fail(inner: &Inner, managed: &Managed, reason: String) {
    tracing::error!("`{}` failed: {reason}", managed.spec.id);
    managed.run().state = ProcessState::Failed;
    let _ = inner.events.send(ProcessEvent::Failed {
        id: managed.spec.id.clone(),
        reason,
    });
}

/// Spawns one generation of `spec` into a new process group.
fn spawn(spec: &ProcessSpec, ledger: &Ledger) -> Result<(Child, ProcessGroup)> {
    let log_err = |source| Error::Io {
        path: spec.log_file.clone(),
        source,
    };
    let exe_err = |source| Error::Io {
        path: spec.program.clone(),
        source,
    };

    let command_line = std::iter::once(spec.program.as_os_str())
        .chain(spec.args.iter().map(|a| a.as_os_str()))
        .map(|s| s.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");
    let stdout =
        logs::open_for_run(&spec.log_file, &format!("Lumina: {command_line}")).map_err(log_err)?;
    let stderr = stdout.try_clone().map_err(log_err)?;

    let mut cmd = std::process::Command::new(&spec.program);
    cmd.args(&spec.args)
        .envs(spec.env.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    if let Some(cwd) = &spec.cwd {
        cmd.current_dir(cwd);
    }
    process_group::configure(&mut cmd);

    let mut child = tokio::process::Command::from(cmd)
        .spawn()
        .map_err(exe_err)?;
    let Some(pid) = child.id() else {
        return Err(Error::Service(spec.id.clone(), "exited immediately".into()));
    };
    #[cfg(windows)]
    let Some(handle) = child.raw_handle() else {
        let _ = child.start_kill();
        return Err(Error::Service(spec.id.clone(), "no process handle".into()));
    };
    let spawned = SpawnedChild {
        pid,
        #[cfg(windows)]
        handle,
    };
    let group = match ProcessGroup::attach(&spawned) {
        Ok(group) => group,
        Err(err) => {
            // On Windows the child is still suspended; it must not linger.
            let _ = child.start_kill();
            return Err(exe_err(err));
        }
    };
    ledger.record(&spec.id, pid, &spec.program);
    Ok((child, group))
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
