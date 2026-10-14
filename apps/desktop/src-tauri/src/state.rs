//! AppState managed by Tauri.
//!
//! `runtime` exists once a data root is known (after the first-run wizard). It
//! owns the configuration, the process supervisor and the services.

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use lumina_config::bootstrap::Bootstrap;
use lumina_config::{AppConfig, ConfigSource, Warning};
use lumina_core::paths::Layout;
use lumina_core::{Error, Result};
use lumina_process::Supervisor;
use lumina_process::ledger::Ledger;
use lumina_services::php_fcgi::PhpFcgi;

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
    pub php: PhpFcgi,
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
        let php = PhpFcgi::new(supervisor.clone(), layout.clone(), templates);
        Ok(Self {
            layout,
            config: RwLock::new(loaded.config),
            warnings: RwLock::new(loaded.warnings),
            config_source: loaded.source,
            supervisor,
            php,
        })
    }

    pub fn config(&self) -> AppConfig {
        self.config
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}
