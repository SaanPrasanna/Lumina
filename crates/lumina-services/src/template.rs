//! Rendering bundled templates (`resources/templates`) into `<data-root>/etc`.
//!
//! Files are written only when their content changes, so a reload is requested
//! only when something actually changed, and every write is atomic so a server
//! never reads a half-written config.

use std::path::{Path, PathBuf};

use lumina_core::{Error, Result};

#[derive(Debug, Clone)]
pub struct Templates {
    dir: PathBuf,
}

impl Templates {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Renders `relative` (e.g. `nginx/nginx.conf.tera`) with `ctx`.
    pub fn render(&self, relative: &str, ctx: &tera::Context) -> Result<String> {
        let path = self.dir.join(relative);
        let source = std::fs::read_to_string(&path).map_err(|source| Error::Io {
            path: path.clone(),
            source,
        })?;
        tera::Tera::one_off(&source, ctx, false).map_err(|e| {
            // Tera hides the useful part in the error chain.
            let mut msg = e.to_string();
            let mut cause = std::error::Error::source(&e);
            while let Some(c) = cause {
                msg.push_str(&format!(": {c}"));
                cause = c.source();
            }
            Error::Config(format!("{}: {msg}", path.display()))
        })
    }

    /// Renders `relative` into `target` if the result differs from what is there.
    pub fn render_to(&self, relative: &str, ctx: &tera::Context, target: &Path) -> Result<bool> {
        write_if_changed(target, self.render(relative, ctx)?.as_bytes())
    }

    /// Copies a bundled file (no rendering) into `target` if it differs.
    pub fn copy_to(&self, relative: &str, target: &Path) -> Result<bool> {
        let path = self.dir.join(relative);
        let bytes = std::fs::read(&path).map_err(|source| Error::Io { path, source })?;
        write_if_changed(target, &bytes)
    }
}

/// Writes `contents` to `path` atomically unless it already has exactly that
/// content. Returns whether the file changed.
pub fn write_if_changed(path: &Path, contents: &[u8]) -> Result<bool> {
    if std::fs::read(path).is_ok_and(|existing| existing == contents) {
        return Ok(false);
    }
    lumina_core::fs::write_atomic(path, contents)?;
    Ok(true)
}

/// Files written during one config update, with what they contained before, so
/// the update can be undone if the server rejects the new config.
#[derive(Debug, Default)]
pub struct ChangeSet {
    previous: Vec<(PathBuf, Option<Vec<u8>>)>,
}

impl ChangeSet {
    /// Like [`write_if_changed`], remembering the old content for [`Self::rollback`].
    pub fn write(&mut self, path: &Path, contents: &[u8]) -> Result<bool> {
        let old = std::fs::read(path).ok();
        if old.as_deref() == Some(contents) {
            return Ok(false);
        }
        lumina_core::fs::write_atomic(path, contents)?;
        self.previous.push((path.to_path_buf(), old));
        Ok(true)
    }

    pub fn changed(&self) -> bool {
        !self.previous.is_empty()
    }

    /// Restores every file written through this change set (newest first).
    pub fn rollback(self) {
        for (path, old) in self.previous.into_iter().rev() {
            let result = match old {
                Some(bytes) => lumina_core::fs::write_atomic(&path, &bytes),
                None => std::fs::remove_file(&path).map_err(|source| Error::Io {
                    path: path.clone(),
                    source,
                }),
            };
            if let Err(err) = result {
                tracing::error!("could not roll back {}: {err}", path.display());
            }
        }
    }
}

impl Templates {
    /// Renders `relative` into `target` through `changes`.
    pub fn render_into(
        &self,
        changes: &mut ChangeSet,
        relative: &str,
        ctx: &tera::Context,
        target: &Path,
    ) -> Result<bool> {
        changes.write(target, self.render(relative, ctx)?.as_bytes())
    }

    /// Copies a bundled file into `target` through `changes`.
    pub fn copy_into(
        &self,
        changes: &mut ChangeSet,
        relative: &str,
        target: &Path,
    ) -> Result<bool> {
        let path = self.dir.join(relative);
        let bytes = std::fs::read(&path).map_err(|source| Error::Io { path, source })?;
        changes.write(target, &bytes)
    }
}

/// Server configs accept forward slashes on every OS and need no escaping then.
pub fn forward_slashes(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_only_when_content_changes() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("a").join("site.conf");

        assert!(write_if_changed(&path, b"one").expect("create"));
        assert!(!write_if_changed(&path, b"one").expect("same"));
        assert!(write_if_changed(&path, b"two").expect("changed"));
        assert_eq!(std::fs::read(&path).expect("read"), b"two");
    }

    #[test]
    fn render_errors_name_the_template() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join("bad.tera"), "{{ missing_var }}").expect("write");
        let err = Templates::new(tmp.path())
            .render("bad.tera", &tera::Context::new())
            .expect_err("missing variable");
        let msg = err.to_string();
        assert!(
            msg.contains("bad.tera") && msg.contains("missing_var"),
            "{msg}"
        );
    }

    #[test]
    fn change_set_rolls_back_new_and_modified_files() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let existing = tmp.path().join("nginx.conf");
        let created = tmp.path().join("sites").join("new.conf");
        std::fs::write(&existing, b"good").expect("write");

        let mut changes = ChangeSet::default();
        assert!(changes.write(&existing, b"broken").expect("modify"));
        assert!(changes.write(&created, b"new").expect("create"));
        assert!(!changes.write(&existing, b"broken").expect("unchanged"));
        assert!(changes.changed());
        changes.rollback();

        assert_eq!(std::fs::read(&existing).expect("read"), b"good");
        assert!(!created.exists());
    }

    #[test]
    fn forward_slashes_everywhere() {
        assert_eq!(
            forward_slashes(Path::new(r"C:\Lumina\www")),
            "C:/Lumina/www"
        );
    }
}
