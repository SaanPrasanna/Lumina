# Contributing to Lumina

1. Fork and create a branch: `feat/<topic>` or `fix/<topic>`.
2. Follow [Conventional Commits](https://www.conventionalcommits.org/) (`feat:`, `fix:`, `docs:` …).
3. Before opening a PR run:
   ```bash
   cargo fmt --all && cargo lint && cargo test --workspace
   pnpm lint && pnpm typecheck && pnpm test
   ```
4. New user-facing strings go in `apps/desktop/src/i18n/locales/en/*.json` only;
   translators update the other locales.
5. Significant design changes need an ADR in `docs/adr/`.

## Where things live

| You want to change…               | Look in                                     |
|-----------------------------------|---------------------------------------------|
| How processes are spawned/killed  | `crates/lumina-process`                     |
| A specific service (PHP, Nginx…)  | `crates/lumina-services/src/<service>`      |
| OS-specific behaviour             | `crates/lumina-platform/src/<area>/<os>.rs` |
| A Tauri command exposed to the UI | `apps/desktop/src-tauri/src/commands`       |
| A UI screen                       | `apps/desktop/src/features/<feature>`       |
| Generated server config files     | `resources/templates`                       |
