//! Watches the `www` root and keeps vhost configs + hosts entries in sync.
//!
//! - [`app`]: projects served by their own dev server (Vite, Next.js, Django).
//! - [`domain`]: folder name -> `<slug>.<tld>`, docroot detection.
//! - [`sites`]: the desired sites for the current folders and config.
//! - [`render`]: which `auto.*.conf` files a web server's `sites/` should hold.
//! - [`sync`]: the hosts file block, changed through the elevated helper.
//! - [`watcher`]: notices new, removed and changed project folders.

pub mod app;
pub mod domain;
pub mod render;
pub mod sites;
pub mod sync;
pub mod watcher;

pub use sites::{AppSite, Scan, Site, scan};
