//! Database snapshots: one database saved to a file and restored later, e.g.
//! before a migration or between test runs. MySQL/MariaDB use `mysqldump`
//! (plain SQL), PostgreSQL `pg_dump`'s custom format; the tools come from the
//! running server's own `bin/` folder, so their versions match.
//!
//! Files live in `data/snapshots/<engine>/<database>/<UTC time>.<sql|dump>`.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use lumina_core::paths::Layout;
use lumina_core::{Error, Result};
use serde::Serialize;
use tokio::process::Command;
use tokio::time::timeout;

const TOOL_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// A running database server, as snapshots need it.
#[derive(Debug, Clone)]
pub struct Server {
    /// `mysql`, `mariadb` or `postgresql`: the folder under `data/snapshots`.
    pub engine: &'static str,
    /// Where the server's own programs are (`mysqldump`, `pg_dump`, ...).
    pub bin: PathBuf,
    pub port: u16,
}

impl Server {
    fn postgres(&self) -> bool {
        self.engine == "postgresql"
    }

    fn tool(&self, names: &[&str]) -> Result<PathBuf> {
        names
            .iter()
            .map(|n| {
                self.bin
                    .join(format!("{n}{}", std::env::consts::EXE_SUFFIX))
            })
            .find(|p| p.is_file())
            .ok_or_else(|| {
                Error::Service(
                    self.engine.into(),
                    format!("{} is not in {}", names.join(" or "), self.bin.display()),
                )
            })
    }

    /// Connection arguments: Lumina's servers listen on 127.0.0.1 with a
    /// password-less admin user.
    fn connect(&self) -> Vec<String> {
        if self.postgres() {
            vec![
                "-h".into(),
                "127.0.0.1".into(),
                "-p".into(),
                self.port.to_string(),
                "-U".into(),
                "postgres".into(),
            ]
        } else {
            vec![
                "--host=127.0.0.1".into(),
                format!("--port={}", self.port),
                "--user=root".into(),
            ]
        }
    }
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// `<engine>/<database>/<file>`, the handle the UI passes back.
    pub id: String,
    pub engine: String,
    pub database: String,
    /// Unix seconds.
    pub created: u64,
    pub size: u64,
}

/// Database names Lumina reads and writes: letters, digits, `_`, `-`, `$`.
/// Anything else is refused, so a name can never act as SQL or a path.
fn check_name(db: &str) -> Result<()> {
    let ok = !db.is_empty()
        && db.len() <= 64
        && db
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '$'));
    if ok {
        Ok(())
    } else {
        Err(Error::Other(format!(
            "`{db}` is not a database name Lumina can snapshot"
        )))
    }
}

pub fn root(layout: &Layout) -> PathBuf {
    layout.data().join("snapshots")
}

async fn run(program: &Path, args: &[String], stdin: Option<&Path>) -> Result<String> {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    // psql/pg_dump must never stop to ask for a password.
    cmd.env("PGCONNECT_TIMEOUT", "10");
    match stdin {
        Some(file) => {
            let f = std::fs::File::open(file).map_err(|source| Error::Io {
                path: file.to_path_buf(),
                source,
            })?;
            cmd.stdin(Stdio::from(f));
        }
        None => {
            cmd.stdin(Stdio::null());
        }
    }
    let out = timeout(TOOL_TIMEOUT, cmd.output())
        .await
        .map_err(|_| Error::Other(format!("{} took too long", program.display())))?
        .map_err(|source| Error::Io {
            path: program.to_path_buf(),
            source,
        })?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(Error::Other(format!(
            "{} failed: {}",
            program.file_name().unwrap_or_default().to_string_lossy(),
            String::from_utf8_lossy(&out.stderr).trim()
        )))
    }
}

/// The server's own databases, without the system ones.
pub async fn databases(server: &Server) -> Result<Vec<String>> {
    let mut names: Vec<String> = if server.postgres() {
        let psql = server.tool(&["psql"])?;
        let mut args = server.connect();
        args.extend(
            [
                "-At",
                "-d",
                "postgres",
                "-c",
                "select datname from pg_database where not datistemplate",
            ]
            .map(String::from),
        );
        run(&psql, &args, None)
            .await?
            .lines()
            .map(str::to_owned)
            .collect()
    } else {
        let mysql = server.tool(&["mysql", "mariadb"])?;
        let mut args = server.connect();
        args.extend(["-N", "-e", "SHOW DATABASES"].map(String::from));
        run(&mysql, &args, None)
            .await?
            .lines()
            .map(str::to_owned)
            .collect()
    };
    let system = [
        "information_schema",
        "mysql",
        "performance_schema",
        "sys",
        "postgres",
    ];
    names.retain(|n| !n.is_empty() && !system.contains(&n.as_str()));
    names.sort();
    Ok(names)
}

/// Saves database `db` of `server` to a new snapshot file.
pub async fn create(layout: &Layout, server: &Server, db: &str) -> Result<Snapshot> {
    check_name(db)?;
    let dir = root(layout).join(server.engine).join(db);
    std::fs::create_dir_all(&dir).map_err(|source| Error::Io {
        path: dir.clone(),
        source,
    })?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let ext = if server.postgres() { "dump" } else { "sql" };
    let file = dir.join(format!("{}.{ext}", stamp(now)));
    let partial = file.with_extension(format!("{ext}.partial"));

    let mut args = server.connect();
    let tool = if server.postgres() {
        args.extend(["-Fc", "-f"].map(String::from));
        args.push(partial.to_string_lossy().into_owned());
        args.push(db.to_owned());
        server.tool(&["pg_dump"])?
    } else {
        args.extend(
            [
                "--single-transaction",
                "--routines",
                "--triggers",
                "--events",
                "--default-character-set=utf8mb4",
            ]
            .map(String::from),
        );
        args.push(format!("--result-file={}", partial.to_string_lossy()));
        args.push(db.to_owned());
        server.tool(&["mysqldump", "mariadb-dump"])?
    };
    if let Err(err) = run(&tool, &args, None).await {
        let _ = std::fs::remove_file(&partial);
        return Err(err);
    }
    std::fs::rename(&partial, &file).map_err(|source| Error::Io {
        path: file.clone(),
        source,
    })?;
    tracing::info!("snapshot of {} {db}: {}", server.engine, file.display());
    describe(layout, &file).ok_or_else(|| Error::Other("the snapshot file is missing".into()))
}

/// Replaces database `snapshot.database` with the snapshot's content.
pub async fn restore(layout: &Layout, server: &Server, id: &str) -> Result<()> {
    let (file, snapshot) = find(layout, id)?;
    if snapshot.engine != server.engine {
        return Err(Error::Other(format!(
            "this is a {} snapshot; start {} to restore it",
            snapshot.engine, snapshot.engine
        )));
    }
    let db = &snapshot.database;
    check_name(db)?;
    let connect = server.connect();
    if server.postgres() {
        let mut drop = connect.clone();
        drop.extend(["--if-exists".into(), "--force".into(), db.clone()]);
        run(&server.tool(&["dropdb"])?, &drop, None).await?;
        let mut create = connect.clone();
        create.push(db.clone());
        run(&server.tool(&["createdb"])?, &create, None).await?;
        let mut args = connect;
        args.extend(["--no-owner".into(), "-d".into(), db.clone()]);
        args.push(file.to_string_lossy().into_owned());
        run(&server.tool(&["pg_restore"])?, &args, None).await?;
    } else {
        let mysql = server.tool(&["mysql", "mariadb"])?;
        let mut reset = connect.clone();
        reset.push("-e".into());
        reset.push(format!(
            "DROP DATABASE IF EXISTS `{db}`; CREATE DATABASE `{db}` CHARACTER SET utf8mb4"
        ));
        run(&mysql, &reset, None).await?;
        let mut args = connect;
        args.push("--default-character-set=utf8mb4".into());
        args.push(db.clone());
        run(&mysql, &args, Some(&file)).await?;
    }
    tracing::info!("restored {} {db} from {id}", server.engine);
    Ok(())
}

pub fn delete(layout: &Layout, id: &str) -> Result<()> {
    let (file, _) = find(layout, id)?;
    std::fs::remove_file(&file).map_err(|source| Error::Io { path: file, source })
}

/// Every snapshot, newest first.
pub fn list(layout: &Layout) -> Vec<Snapshot> {
    let mut out = Vec::new();
    let root = root(layout);
    for engine in ["mysql", "mariadb", "postgresql"] {
        let Ok(dbs) = std::fs::read_dir(root.join(engine)) else {
            continue;
        };
        for db in dbs.flatten() {
            let Ok(files) = std::fs::read_dir(db.path()) else {
                continue;
            };
            out.extend(files.flatten().filter_map(|f| describe(layout, &f.path())));
        }
    }
    out.sort_by(|a, b| b.created.cmp(&a.created).then_with(|| b.id.cmp(&a.id)));
    out
}

fn describe(layout: &Layout, file: &Path) -> Option<Snapshot> {
    let ext = file.extension()?.to_str()?;
    if ext != "sql" && ext != "dump" {
        return None;
    }
    let rel = file.strip_prefix(root(layout)).ok()?;
    let mut parts = rel.iter().map(|p| p.to_string_lossy().into_owned());
    let (engine, database, name) = (parts.next()?, parts.next()?, parts.next()?);
    let meta = std::fs::metadata(file).ok()?;
    let created = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs());
    Some(Snapshot {
        id: format!("{engine}/{database}/{name}"),
        engine,
        database,
        created,
        size: meta.len(),
    })
}

/// The file of snapshot `id`, which must be one [`list`] returns (so no
/// path outside `data/snapshots` can be named).
fn find(layout: &Layout, id: &str) -> Result<(PathBuf, Snapshot)> {
    let snapshot = list(layout)
        .into_iter()
        .find(|s| s.id == id)
        .ok_or_else(|| Error::Other(format!("there is no snapshot {id}")))?;
    let file = root(layout)
        .join(&snapshot.engine)
        .join(&snapshot.database)
        .join(id.rsplit('/').next().unwrap_or_default());
    Ok((file, snapshot))
}

/// Unix seconds → `20261018-101500` (UTC), sortable and safe in file names.
fn stamp(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}{month:02}{day:02}-{:02}{:02}{:02}",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamps_are_utc_dates() {
        assert_eq!(stamp(0), "19700101-000000");
        assert_eq!(stamp(1_792_318_500), "20261018-101500");
        assert_eq!(stamp(951_782_400), "20000229-000000");
    }

    #[test]
    fn only_plain_database_names_are_accepted() {
        for ok in ["blog", "shop_2", "my-app", "a$b"] {
            assert!(check_name(ok).is_ok(), "{ok}");
        }
        for bad in ["", "a b", "x`; DROP", "../etc", "a/b", "a.b"] {
            assert!(check_name(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn lists_snapshot_files_newest_first_and_finds_only_those() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let layout = Layout::new(tmp.path().join("root"));
        let dir = root(&layout).join("mysql").join("blog");
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(dir.join("20261018-101500.sql"), "-- dump").expect("sql");
        std::fs::write(dir.join("notes.txt"), "x").expect("txt");
        std::fs::write(dir.join("20261018-101600.sql.partial"), "x").expect("partial");

        let all = list(&layout);
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, "mysql/blog/20261018-101500.sql");
        assert_eq!(
            (all[0].engine.as_str(), all[0].database.as_str()),
            ("mysql", "blog")
        );
        assert!(find(&layout, "mysql/blog/../../../../etc/passwd").is_err());
        delete(&layout, &all[0].id).expect("delete");
        assert!(list(&layout).is_empty());
    }
}
