//! Well-known defaults. Values here are the fallback when `config.toml` omits a key.

/// PHP FastCGI port. Chosen to avoid Xdebug's 9000/9003.
pub const DEFAULT_PHP_FCGI_PORT: u16 = 10987;
/// Used when [`DEFAULT_PHP_FCGI_PORT`] is already taken.
pub const FALLBACK_PHP_FCGI_PORT: u16 = 20987;
/// Default `PHP_FCGI_CHILDREN`.
pub const DEFAULT_PHP_FCGI_CHILDREN: u32 = 10;
/// Lower bound enforced on `PHP_FCGI_CHILDREN`.
pub const MIN_PHP_FCGI_CHILDREN: u32 = 4;
/// Default `PHP_FCGI_MAX_REQUESTS` (0 = unlimited).
pub const DEFAULT_PHP_FCGI_MAX_REQUESTS: u32 = 0;
/// Default top-level domain for auto virtual hosts.
pub const DEFAULT_TLD: &str = "test";

/// UI languages shipped with the app (BCP 47 codes). Must match
/// apps/desktop/src/i18n/index.ts. More languages are added after 1.0.
pub const SUPPORTED_LANGUAGES: [&str; 2] = ["en", "si"];
/// Language used when none is chosen.
pub const DEFAULT_LANGUAGE: &str = "en";
