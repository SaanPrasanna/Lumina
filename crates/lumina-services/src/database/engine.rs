//! What differs between MySQL and MariaDB: executable names and how a new data
//! directory is created. Everything else is shared by `Database`.

use lumina_config::schema::DatabaseEngine;
use lumina_core::types::ServiceKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Engine {
    pub kind: DatabaseEngine,
    /// Supervisor ID, `bin/<id>/`, `etc/<id>/` and the `data/<id>-x.y` prefix.
    pub id: &'static str,
    pub name: &'static str,
    /// Server executables, relative to the install folder, in order of preference.
    pub server: &'static [&'static str],
    /// Admin tool used for the graceful `shutdown`.
    pub admin: &'static [&'static str],
    /// Process name of the server (for process counts).
    pub process: &'static str,
}

pub const MYSQL: Engine = Engine {
    kind: DatabaseEngine::Mysql,
    id: "mysql",
    name: "MySQL",
    server: &["bin/mysqld", "sbin/mysqld"],
    admin: &["bin/mysqladmin"],
    process: "mysqld",
};

pub const MARIADB: Engine = Engine {
    kind: DatabaseEngine::Mariadb,
    id: "mariadb",
    name: "MariaDB",
    server: &["bin/mariadbd", "sbin/mariadbd", "bin/mysqld"],
    admin: &["bin/mariadb-admin", "bin/mysqladmin"],
    process: "mariadbd",
};

impl Engine {
    pub fn of(kind: DatabaseEngine) -> Self {
        match kind {
            DatabaseEngine::Mysql => MYSQL,
            DatabaseEngine::Mariadb => MARIADB,
        }
    }

    pub fn service_kind(&self) -> ServiceKind {
        match self.kind {
            DatabaseEngine::Mysql => ServiceKind::Mysql,
            DatabaseEngine::Mariadb => ServiceKind::Mariadb,
        }
    }
}

/// "mysqld  Ver 8.4.11 for Win64 ..." / "mariadbd  Ver 12.3.3-MariaDB ..." → version.
pub fn parse_version(text: &str) -> Option<String> {
    let rest = &text[text.find("Ver ")? + 4..];
    let version: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    (!version.is_empty()).then_some(version)
}

/// "8.4.11" → "8.4": data directories are per release line, so patch updates
/// keep their data and a new line (8.4 → 9.1) never touches the old files.
pub fn release_line(version: &str) -> String {
    version.split('.').take(2).collect::<Vec<_>>().join(".")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_both_banners() {
        assert_eq!(
            parse_version(
                "C:\\x\\mysqld.exe  Ver 8.4.11 for Win64 on x86_64 (MySQL Community Server - GPL)"
            )
            .as_deref(),
            Some("8.4.11")
        );
        assert_eq!(
            parse_version("mariadbd.exe  Ver 12.3.3-MariaDB for Win64 on AMD64 (MariaDB Server)")
                .as_deref(),
            Some("12.3.3")
        );
        assert_eq!(parse_version("nope"), None);
    }

    #[test]
    fn release_line_is_major_minor() {
        assert_eq!(release_line("8.4.11"), "8.4");
        assert_eq!(release_line("12.3.3"), "12.3");
    }
}
