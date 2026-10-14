# Tests

- Unit tests live next to code (`#[cfg(test)]` in each crate, `*.test.tsx` in the UI).
- `tests/integration/` — cross-crate Rust tests (e.g. spawn a fake php-cgi, assert no ghost processes after stop).
- `tests/e2e/` — WebDriver tests against the built app via `tauri-driver` (Windows/Linux).
- `tests/fixtures/` — sample sites, fake binaries, config files.
