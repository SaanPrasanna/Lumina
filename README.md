# Lumina

A fast, cross-platform, open-source local development environment — a Laragon-style
stack manager built with **Rust + Tauri 2 + React**.

- One-click Nginx / Apache, PHP (FastCGI), MySQL / MariaDB, Redis, Node.js, Python
- Auto virtual hosts (`*.test`) with trusted local HTTPS
- Version manager (Quick Add) and switchable stack profiles
- QuickApp installers (Laravel, WordPress, …), Ngrok sharing, global PATH injection
- Windows, macOS and Linux

> Status: early development. See [`IMPLEMENTATION_PLAN.md`](IMPLEMENTATION_PLAN.md) for the
> roadmap and [`docs/`](docs/) for the architecture.

## Repository layout

```
apps/desktop/          Tauri app (React UI in src/, Rust shell in src-tauri/)
crates/                Rust domain crates (process mgmt, services, vhost, ssl, …)
resources/             Files bundled into the app (config templates, catalogs)
packaging/             Per-OS installer configuration (NSIS, macOS, Linux)
assets/                Source artwork (master app icon, screenshots)
xtask/                 Rust build/dev automation (`cargo xtask …`)
scripts/               Shell/PowerShell helpers used by CI and developers
tests/                 Cross-crate integration and end-to-end tests
docs/                  Architecture, ADRs, guides
```

## Quick start

```bash
corepack enable          # provides pnpm
pnpm install
pnpm dev                 # runs the Tauri app with hot reload
cargo test --workspace   # Rust tests
```

See [docs/guides/development.md](docs/guides/development.md) for platform prerequisites.

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
