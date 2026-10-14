//! Versioned migrations so app updates never lose user settings.
//!
//! Each step upgrades the raw TOML from version N to N+1, before it is parsed
//! into [`AppConfig`](crate::schema::AppConfig). To add one: bump
//! `CURRENT_SCHEMA_VERSION`, add a `N => step_N_to_N1(&mut table)` arm, and a test.

use lumina_core::{Error, Result};
use toml::Value;

use crate::schema::CURRENT_SCHEMA_VERSION;

/// Upgrades `value` to the current schema. Returns whether anything changed.
pub fn migrate(value: &mut Value) -> Result<bool> {
    let Some(table) = value.as_table_mut() else {
        return Err(Error::Config("config.toml is not a table".into()));
    };
    // Files without a version predate versioning and are treated as version 1.
    let mut version = match table.get("schema_version") {
        None => 1,
        Some(v) => v
            .as_integer()
            .and_then(|n| u32::try_from(n).ok())
            .ok_or_else(|| Error::Config("schema_version must be a positive integer".into()))?,
    };

    if version > CURRENT_SCHEMA_VERSION {
        return Err(Error::Config(format!(
            "config.toml was written by a newer Lumina (schema {version}, this version \
             understands {CURRENT_SCHEMA_VERSION}); update Lumina"
        )));
    }

    let start = version;
    while version < CURRENT_SCHEMA_VERSION {
        // match version { 1 => step_1_to_2(table), ... }
        version += 1;
    }
    table.insert(
        "schema_version".into(),
        Value::Integer(i64::from(CURRENT_SCHEMA_VERSION)),
    );
    Ok(version != start)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Value {
        toml::from_str(s).expect("valid toml")
    }

    #[test]
    fn current_version_is_untouched() {
        let mut v = parse("schema_version = 1\n[php]\nfcgi_children = 8\n");
        assert!(!migrate(&mut v).expect("migrate"));
        assert_eq!(v["php"]["fcgi_children"].as_integer(), Some(8));
    }

    #[test]
    fn missing_version_is_treated_as_one() {
        let mut v = parse("[php]\nfcgi_children = 8\n");
        migrate(&mut v).expect("migrate");
        assert_eq!(v["schema_version"].as_integer(), Some(1));
    }

    #[test]
    fn newer_schema_is_refused() {
        let mut v = parse("schema_version = 99\n");
        assert!(matches!(migrate(&mut v), Err(Error::Config(msg)) if msg.contains("newer")));
    }
}
