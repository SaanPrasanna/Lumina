//! Repairs what can be repaired and reports problems as [`Warning`]s for the UI.
//! Repairs only change the in-memory config; the user's file is left untouched
//! until they save settings in the app.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::defaults;
use crate::schema::{AppConfig, Editor};

/// A config problem shown to the user. `code` is a stable i18n key
/// (`configWarnings.<code>` in the locale files); `values` fill its placeholders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Warning {
    pub code: &'static str,
    /// Dotted config path, e.g. `php.fcgi_children`.
    pub field: String,
    /// English text, for logs.
    pub message: String,
    pub values: BTreeMap<&'static str, String>,
}

impl Warning {
    fn new(code: &'static str, field: &str, message: String) -> Self {
        Self {
            code,
            field: field.to_owned(),
            message,
            values: BTreeMap::new(),
        }
    }

    fn with(mut self, key: &'static str, value: impl ToString) -> Self {
        self.values.insert(key, value.to_string());
        self
    }
}

pub fn validate(cfg: &mut AppConfig) -> Vec<Warning> {
    let mut warnings = Vec::new();

    // PHP_FCGI_CHILDREN below the minimum makes PHP drop requests under load.
    if cfg.php.fcgi_children < defaults::MIN_PHP_FCGI_CHILDREN {
        let from = cfg.php.fcgi_children;
        let to = defaults::MIN_PHP_FCGI_CHILDREN;
        cfg.php.fcgi_children = to;
        warnings.push(
            Warning::new(
                "php_children_raised",
                "php.fcgi_children",
                format!("PHP_FCGI_CHILDREN={from} is too low; using {to}"),
            )
            .with("from", from)
            .with("to", to),
        );
    }

    reset_zero_port(
        &mut cfg.php.fcgi_port,
        "php.fcgi_port",
        defaults::PHP_FCGI_PORT,
        &mut warnings,
    );
    reset_zero_port(
        &mut cfg.php.fcgi_fallback_port,
        "php.fcgi_fallback_port",
        defaults::PHP_FCGI_FALLBACK_PORT,
        &mut warnings,
    );
    reset_zero_port(&mut cfg.web.http_port, "web.http_port", 80, &mut warnings);
    reset_zero_port(
        &mut cfg.web.https_port,
        "web.https_port",
        443,
        &mut warnings,
    );
    reset_zero_port(&mut cfg.database.port, "database.port", 3306, &mut warnings);
    reset_zero_port(&mut cfg.redis.port, "redis.port", 6379, &mut warnings);

    if cfg.php.fcgi_fallback_port == cfg.php.fcgi_port {
        warnings.push(
            Warning::new(
                "php_ports_same",
                "php.fcgi_fallback_port",
                format!(
                    "the PHP fallback port equals the main port ({}); there is no fallback",
                    cfg.php.fcgi_port
                ),
            )
            .with("port", cfg.php.fcgi_port),
        );
    }

    // Services that would fight over one port. The PHP fallback is excluded: it
    // is only used when the main port is taken.
    let mut ports: Vec<(&str, u16)> = vec![
        ("php.fcgi_port", cfg.php.fcgi_port),
        ("web.http_port", cfg.web.http_port),
        ("web.https_port", cfg.web.https_port),
        ("database.port", cfg.database.port),
    ];
    if cfg.redis.enabled {
        ports.push(("redis.port", cfg.redis.port));
    }
    for (i, (field_a, port_a)) in ports.iter().enumerate() {
        for (field_b, port_b) in &ports[i + 1..] {
            if port_a == port_b {
                warnings.push(
                    Warning::new(
                        "port_conflict",
                        field_b,
                        format!("{field_a} and {field_b} both use port {port_a}"),
                    )
                    .with("port", port_a)
                    .with("a", field_a)
                    .with("b", field_b),
                );
            }
        }
    }

    // "Test" and ".test" mean "test".
    let tld = cfg
        .general
        .tld
        .trim()
        .trim_start_matches('.')
        .to_ascii_lowercase();
    if is_valid_tld(&tld) {
        cfg.general.tld = tld;
    }
    if !is_valid_tld(&cfg.general.tld) {
        warnings.push(
            Warning::new(
                "tld_reset",
                "general.tld",
                format!(
                    "invalid domain suffix {:?}; using .{}",
                    cfg.general.tld,
                    defaults::TLD
                ),
            )
            .with("value", &cfg.general.tld),
        );
        cfg.general.tld = defaults::TLD.to_owned();
    }

    if HTTPS_ONLY_TLDS.contains(&cfg.general.tld.as_str()) {
        warnings.push(
            Warning::new(
                "tld_https_only",
                "general.tld",
                format!(
                    ".{} is a real domain suffix that browsers only open over HTTPS",
                    cfg.general.tld
                ),
            )
            .with("tld", &cfg.general.tld),
        );
    }

    // Profiles need a unique, non-empty name: the UI and the tray switch by name.
    let mut seen = std::collections::BTreeSet::new();
    let before = cfg.profiles.list.len();
    cfg.profiles.list.retain_mut(|p| {
        p.name = p.name.trim().to_owned();
        !p.name.is_empty() && seen.insert(p.name.to_lowercase())
    });
    let dropped = before - cfg.profiles.list.len();
    if dropped > 0 {
        warnings.push(
            Warning::new(
                "profiles_dropped",
                "profiles.list",
                format!(
                    "{dropped} profile(s) without a name or with a duplicate name were ignored"
                ),
            )
            .with("count", dropped),
        );
    }

    if cfg.editor.command.trim().is_empty() {
        cfg.editor = Editor::default();
        warnings.push(Warning::new(
            "editor_reset",
            "editor.command",
            "no editor command set; using VS Code".to_owned(),
        ));
    }

    warnings
}

fn reset_zero_port(port: &mut u16, field: &str, default: u16, warnings: &mut Vec<Warning>) {
    if *port == 0 {
        *port = default;
        warnings.push(
            Warning::new(
                "port_zero_reset",
                field,
                format!("{field} cannot be 0; using {default}"),
            )
            .with("default", default),
        );
    }
}

/// Real top-level domains on the browsers' HSTS preload list: plain-HTTP sites
/// under them never load.
const HTTPS_ONLY_TLDS: &[&str] = &[
    "app", "bank", "boo", "channel", "dad", "day", "dev", "esq", "foo", "ing", "meme", "mov",
    "new", "nexus", "page", "phd", "prof", "rsvp", "zip",
];

/// A single DNS label: letters, digits and inner hyphens.
fn is_valid_tld(tld: &str) -> bool {
    !tld.is_empty()
        && tld.len() <= 63
        && tld
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !tld.starts_with('-')
        && !tld.ends_with('-')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(w: &[Warning]) -> Vec<&str> {
        w.iter().map(|w| w.code).collect()
    }

    #[test]
    fn defaults_are_valid() {
        let mut cfg = AppConfig::default();
        assert!(validate(&mut cfg).is_empty());
        assert_eq!(cfg, AppConfig::default());
    }

    #[test]
    fn raises_children_to_minimum_with_warning() {
        let mut cfg = AppConfig::default();
        cfg.php.fcgi_children = 2;

        let warnings = validate(&mut cfg);

        assert_eq!(cfg.php.fcgi_children, 4);
        assert_eq!(codes(&warnings), ["php_children_raised"]);
        assert_eq!(warnings[0].values["from"], "2");
        assert_eq!(warnings[0].values["to"], "4");
    }

    #[test]
    fn resets_zero_ports() {
        let mut cfg = AppConfig::default();
        cfg.php.fcgi_port = 0;
        let warnings = validate(&mut cfg);
        assert_eq!(cfg.php.fcgi_port, 10987);
        assert_eq!(codes(&warnings), ["port_zero_reset"]);
    }

    #[test]
    fn reports_port_conflicts_but_ignores_disabled_redis() {
        let mut cfg = AppConfig::default();
        cfg.database.port = 80;
        cfg.redis.port = 443; // redis disabled: no conflict
        let warnings = validate(&mut cfg);
        assert_eq!(codes(&warnings), ["port_conflict"]);
        assert_eq!(warnings[0].field, "database.port");
    }

    #[test]
    fn same_main_and_fallback_port_is_reported() {
        let mut cfg = AppConfig::default();
        cfg.php.fcgi_fallback_port = cfg.php.fcgi_port;
        assert_eq!(codes(&validate(&mut cfg)), ["php_ports_same"]);
    }

    #[test]
    fn repairs_tld_and_editor() {
        let mut cfg = AppConfig::default();
        cfg.general.tld = ".bad tld".into();
        cfg.editor.command = "  ".into();
        let warnings = validate(&mut cfg);
        assert_eq!(cfg.general.tld, "test");
        assert_eq!(cfg.editor, Editor::default());
        assert_eq!(codes(&warnings), ["tld_reset", "editor_reset"]);
    }

    #[test]
    fn profiles_need_unique_names() {
        use crate::schema::Profile;
        let mut cfg = AppConfig::default();
        let named = |n: &str| Profile {
            name: n.into(),
            ..Profile::default()
        };
        cfg.profiles.list = vec![
            named(" Legacy "),
            named("legacy"),
            named(""),
            named("Modern"),
        ];
        let warnings = validate(&mut cfg);
        assert_eq!(codes(&warnings), ["profiles_dropped"]);
        let names: Vec<&str> = cfg.profiles.list.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Legacy", "Modern"]);
    }

    #[test]
    fn normalises_tld_and_warns_about_https_only_suffixes() {
        let mut cfg = AppConfig::default();
        cfg.general.tld = " .Local".into();
        assert!(validate(&mut cfg).is_empty());
        assert_eq!(cfg.general.tld, "local");

        cfg.general.tld = "dev".into();
        assert_eq!(codes(&validate(&mut cfg)), ["tld_https_only"]);
        assert_eq!(cfg.general.tld, "dev");
    }
}
