//! Default values, sourced from `lumina_core::constants` so there is one source
//! of truth shared with the services.

use lumina_core::constants;

pub const TLD: &str = constants::DEFAULT_TLD;
pub const PHP_FCGI_PORT: u16 = constants::DEFAULT_PHP_FCGI_PORT;
pub const PHP_FCGI_FALLBACK_PORT: u16 = constants::FALLBACK_PHP_FCGI_PORT;
pub const PHP_FCGI_CHILDREN: u32 = constants::DEFAULT_PHP_FCGI_CHILDREN;
pub const PHP_FCGI_MAX_REQUESTS: u32 = constants::DEFAULT_PHP_FCGI_MAX_REQUESTS;
pub const MIN_PHP_FCGI_CHILDREN: u32 = constants::MIN_PHP_FCGI_CHILDREN;

/// The commented template written on first run (resources/config/default.toml).
pub const DEFAULT_TOML: &str = include_str!("../../../resources/config/default.toml");
