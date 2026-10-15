//! `config.toml`: schema, defaults, load/save,
//! validation and migrations, plus the bootstrap file that points at the data root.

pub mod bootstrap;
pub mod defaults;
pub mod loader;
pub mod migrate;
pub mod schema;
pub mod validate;

pub use loader::{ConfigSource, Loaded, backup_before_upgrade, load_or_create, save};
pub use schema::AppConfig;
pub use validate::Warning;
