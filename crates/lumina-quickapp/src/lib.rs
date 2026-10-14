//! QuickApp: one-click projects (Laravel, WordPress, Symfony, ...) from
//! declarative recipes (`resources/quickapp/recipes.toml`), run step by step
//! with Lumina's own PHP, Composer, Node.js and database.

pub mod files;
pub mod recipe;
pub mod runner;

use std::path::{Path, PathBuf};

pub use recipe::{Need, Recipe, RecipeFile};
pub use runner::{Context, Event, Report, Tool};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("recipe error: {0}")]
    Recipe(String),
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{program} failed{}{}", code.map(|c| format!(" (exit code {c})")).unwrap_or_default(), detail.as_ref().map(|d| format!(": {d}")).unwrap_or_default())]
    Command {
        program: String,
        code: Option<i32>,
        detail: Option<String>,
    },
    #[error("download failed: {0}")]
    Download(String),
    #[error("database: {0}")]
    Database(String),
    #[error("cancelled")]
    Cancelled,
    #[error("{0}")]
    Other(String),
}

impl From<Error> for lumina_core::Error {
    fn from(e: Error) -> Self {
        match e {
            Error::Io { path, source } => lumina_core::Error::Io { path, source },
            other => lumina_core::Error::Other(other.to_string()),
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// `resources/quickapp/recipes.toml`.
pub fn bundled(resources: &Path) -> Result<RecipeFile> {
    let path = resources.join("quickapp").join("recipes.toml");
    let text = std::fs::read_to_string(&path).map_err(|source| Error::Io { path, source })?;
    RecipeFile::parse(&text)
}
