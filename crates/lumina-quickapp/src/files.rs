//! Pure helpers for the file steps: dotenv patching, names, SQL.

use std::collections::BTreeMap;

/// Sets every key of `set` in a dotenv-style `text`.
///
/// A key is matched on lines `KEY=...`, `KEY = ...` and their commented-out
/// forms (`# KEY=...`, as in `.env.example` files and CodeIgniter's `env`);
/// the first match is replaced, uncommented, keeping its spacing around `=`.
/// Later duplicates are left alone. Keys that are not present are appended.
/// Line endings follow the file.
pub fn patch_env(text: &str, set: &BTreeMap<String, String>) -> String {
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    for (key, value) in set {
        let mut found = None;
        for (i, line) in lines.iter().enumerate() {
            let Some(sep) = key_separator(line, key) else {
                continue;
            };
            let active = !line.trim_start().starts_with('#');
            // An active line wins over a commented-out one that came first.
            if active || found.is_none() {
                found = Some((i, sep));
            }
            if active {
                break;
            }
        }
        match found {
            Some((i, sep)) => lines[i] = format!("{key}{sep}{value}"),
            None => lines.push(format!("{key}={value}")),
        }
    }
    let mut out = lines.join(newline);
    out.push_str(newline);
    out
}

/// The separator (`=`, ` = `) if `line` assigns `key`, commented or not.
fn key_separator(line: &str, key: &str) -> Option<String> {
    let body = line.trim_start();
    let body = body.strip_prefix('#').map_or(body, str::trim_start);
    let rest = body.strip_prefix(key)?;
    let eq = rest.find('=')?;
    let before = &rest[..eq];
    if !before.chars().all(char::is_whitespace) {
        return None; // e.g. `DB_HOST_EXTRA=` when looking for `DB_HOST`
    }
    let after = &rest[eq + 1..];
    let pad = |s: &str| if s.is_empty() { "" } else { " " };
    let trailing = after.len() - after.trim_start().len();
    Some(format!("{}={}", pad(before), pad(&after[..trailing])))
}

/// A project name: lowercase letters, digits and inner dashes, at most 63
/// characters, so it is a folder, a host name (`name.test`) and (with `_`
/// for `-`) a database name at once.
pub fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 63
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !name.starts_with('-')
        && !name.ends_with('-')
}

/// The database for project `name`: what the Laravel installer also picks
/// (`my-shop` → `my_shop`).
pub fn database_name(name: &str) -> String {
    name.replace('-', "_")
}

/// Database names Lumina creates: letters, digits and `_` only, so they
/// need no quoting in SQL.
pub fn is_safe_database(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Shell-free SQL: one row `<exists> <tables>` for `db`.
pub fn sql_inspect(db: &str) -> String {
    format!(
        "SELECT (SELECT COUNT(*) FROM information_schema.schemata WHERE schema_name = '{db}'), \
         (SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = '{db}');"
    )
}

pub fn sql_create(db: &str) -> String {
    format!("CREATE DATABASE `{db}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;")
}

pub fn sql_drop(db: &str) -> String {
    format!("DROP DATABASE IF EXISTS `{db}`;")
}

/// Removes ANSI escape sequences (colours, cursor moves) from tool output.
pub fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        match chars.peek() {
            // CSI: ESC [ params final-byte(@..~)
            Some('[') => {
                chars.next();
                for c in chars.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
            // OSC: ESC ] ... BEL or ESC \
            Some(']') => {
                chars.next();
                while let Some(c) = chars.next() {
                    if c == '\u{7}' || (c == '\u{1b}' && chars.next_if_eq(&'\\').is_some()) {
                        break;
                    }
                }
            }
            Some(_) => {
                chars.next();
            }
            None => {}
        }
    }
    out
}

/// `*` wildcard match (GitHub release asset names).
pub fn glob_match(pattern: &str, name: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == name;
    }
    let (first, last) = (parts[0], parts[parts.len() - 1]);
    if !name.starts_with(first) || !name.ends_with(last) || name.len() < first.len() + last.len() {
        return false;
    }
    let mut rest = &name[first.len()..name.len() - last.len()];
    for part in &parts[1..parts.len() - 1] {
        match rest.find(part) {
            Some(i) => rest = &rest[i + part.len()..],
            None => return false,
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).into(), (*v).into()))
            .collect()
    }

    #[test]
    fn patches_laravel_env() {
        let env = "APP_NAME=Laravel\nAPP_URL=http://localhost\n\nDB_CONNECTION=sqlite\n# DB_HOST=127.0.0.1\n# DB_PORT=3306\nDB_HOST_EXTRA=1\n";
        let out = patch_env(
            env,
            &set(&[
                ("APP_URL", "https://blog.test"),
                ("DB_CONNECTION", "mysql"),
                ("DB_HOST", "127.0.0.1"),
                ("DB_PORT", "3307"),
                ("DB_DATABASE", "blog"),
            ]),
        );
        assert_eq!(
            out,
            "APP_NAME=Laravel\nAPP_URL=https://blog.test\n\nDB_CONNECTION=mysql\nDB_HOST=127.0.0.1\nDB_PORT=3307\nDB_HOST_EXTRA=1\nDB_DATABASE=blog\n"
        );
    }

    #[test]
    fn patches_spaced_and_commented_keys() {
        let env = "# CI_ENVIRONMENT = production\r\n# app.baseURL = ''\r\n";
        let out = patch_env(
            env,
            &set(&[
                ("CI_ENVIRONMENT", "development"),
                ("app.baseURL", "'https://ci.test/'"),
            ]),
        );
        assert_eq!(
            out,
            "CI_ENVIRONMENT = development\r\napp.baseURL = 'https://ci.test/'\r\n"
        );
    }

    #[test]
    fn active_line_beats_commented_one() {
        let out = patch_env("#DB_PORT=1\nDB_PORT=2\n", &set(&[("DB_PORT", "3")]));
        assert_eq!(out, "#DB_PORT=1\nDB_PORT=3\n");
    }

    #[test]
    fn names() {
        for ok in ["blog", "my-shop", "a1", "x"] {
            assert!(is_valid_name(ok), "{ok}");
        }
        for bad in ["", "-a", "a-", "My", "a_b", "a.b", "a b", &"a".repeat(64)] {
            assert!(!is_valid_name(bad), "{bad}");
        }
        assert_eq!(database_name("my-shop"), "my_shop");
        assert!(is_safe_database("my_shop"));
        assert!(!is_safe_database("x`; DROP"));
    }

    #[test]
    fn ansi_is_removed() {
        assert_eq!(
            strip_ansi("\u{1b}[32mok\u{1b}[0m done\u{1b}]8;;http://x\u{7}"),
            "ok done"
        );
    }

    #[test]
    fn globs() {
        assert!(glob_match(
            "Joomla_*-Stable-Full_Package.zip",
            "Joomla_5.3.2-Stable-Full_Package.zip"
        ));
        assert!(!glob_match(
            "Joomla_*-Stable-Full_Package.zip",
            "Joomla_5.3.2-Stable-Update_Package.zip"
        ));
        assert!(glob_match("a*b*c", "aXbYc"));
        assert!(!glob_match("a*a", "a"));
        assert!(glob_match("exact", "exact"));
    }
}
