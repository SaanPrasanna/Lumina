//! AppState managed by Tauri.
//!
//! `runtime` exists once a data root is known (after the first-run wizard). It
//! owns the configuration, the process supervisor and the services.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use lumina_config::bootstrap::Bootstrap;
use lumina_config::{AppConfig, ConfigSource, Warning};
use lumina_core::paths::Layout;
use lumina_core::{Error, Result};
use lumina_process::Supervisor;
use lumina_process::ledger::Ledger;
use lumina_services::orchestrator::Orchestrator;
use lumina_services::template::Templates;
use lumina_vhost::sync::HostsSync;
use lumina_vhost::watcher::Watcher;

pub struct AppState {
    /// `<app-config-dir>/bootstrap.toml`.
    pub bootstrap_path: PathBuf,
    /// `None` until the first-run wizard completes.
    pub bootstrap: RwLock<Option<Bootstrap>>,
    /// Bundled `resources/templates`.
    pub templates: PathBuf,
    pub runtime: RwLock<Option<Arc<Runtime>>>,
}

impl AppState {
    /// The runtime, or a `not_configured` error before the first-run wizard.
    pub fn runtime(&self) -> Result<Arc<Runtime>> {
        self.runtime
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .ok_or_else(|| Error::Config("Lumina is not set up yet".into()))
    }
}

pub struct Runtime {
    pub layout: Layout,
    pub config: RwLock<AppConfig>,
    /// Problems found when the config was last loaded or saved.
    pub warnings: RwLock<Vec<Warning>>,
    pub config_source: ConfigSource,
    pub supervisor: Supervisor,
    /// PHP + web server as one stack.
    pub stack: Orchestrator,
    /// The hosts file's Lumina block, changed through `lumina-helper`.
    pub hosts: HostsSync,
    /// `lumina-helper`, for privileged changes (hosts file; CA on Linux).
    pub helper: PathBuf,
    /// Watches the projects folder (and which folder it watches).
    pub watcher: Mutex<Option<(PathBuf, Watcher)>>,
}

/// `lumina-helper` sits next to the app executable, in development
/// (`target/debug`) and in installed builds alike.
fn helper_path() -> PathBuf {
    let name = format!("lumina-helper{}", std::env::consts::EXE_SUFFIX);
    std::env::current_exe()
        .map(|exe| exe.with_file_name(&name))
        .unwrap_or_else(|_| PathBuf::from(name))
}

impl Runtime {
    /// Loads config.toml, kills leftovers of a crashed previous run, and creates
    /// the supervisor and services. Nothing is started.
    pub fn init(layout: Layout, templates: PathBuf) -> Result<Self> {
        layout.ensure()?;
        let loaded = lumina_config::load_or_create(&layout.config_file())?;

        let ledger = Ledger::open(layout.tmp().join("pids.json"));
        for leftover in ledger.sweep(&layout.bin()) {
            log::warn!(
                "killed leftover `{}` (pid {}) from a previous run",
                leftover.id,
                leftover.pid
            );
        }

        let supervisor = Supervisor::new(ledger);
        let stack = Orchestrator::new(
            supervisor.clone(),
            layout.clone(),
            Templates::new(templates),
        );
        let helper = helper_path();
        if !helper.is_file() {
            log::warn!(
                "{} is missing; hosts entries cannot be written (build it with `cargo build -p lumina-helper`)",
                helper.display()
            );
        }
        let hosts = HostsSync::new(
            helper.clone(),
            lumina_platform::hosts::system_hosts_path(),
            layout.tmp(),
        );
        Ok(Self {
            hosts,
            helper,
            watcher: Mutex::new(None),
            layout,
            config: RwLock::new(loaded.config),
            warnings: RwLock::new(loaded.warnings),
            config_source: loaded.source,
            supervisor,
            stack,
        })
    }

    pub fn config(&self) -> AppConfig {
        self.config
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}
