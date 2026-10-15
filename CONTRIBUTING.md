# Contributing to Lumina

Thank you for helping. Bug reports, translations, documentation and code are
all welcome. This guide explains how to set up the project, what every change
must pass, and how pull requests are reviewed.

By taking part you agree to follow the [Code of Conduct](CODE_OF_CONDUCT.md).
Security problems are not reported here: see [SECURITY.md](SECURITY.md).

## Ways to contribute

- **Report a bug.** Use the [bug report form](https://github.com/SaanPrasanna/Lumina/issues/new/choose).
  Include your system, the Lumina version (Settings → About and updates), what you did,
  what you expected, and the relevant lines from the Logs page or
  `lumina.log`. Search existing issues first.
- **Suggest a feature.** Use the feature request form and describe the problem
  it solves, not only the solution.
- **Translate.** The app ships in English and Sinhala. Corrections to the
  Sinhala text are very welcome.
- **Write code.** For anything larger than a small fix, open an issue first so
  we can agree on the approach before you spend time on it.

## Development setup

You need:

- [Rust](https://rustup.rs/), stable (`rust-toolchain.toml` selects it).
- [Node.js](https://nodejs.org/) 22 or later. pnpm comes through Corepack.
- The [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your
  system. On Debian/Ubuntu:
  ```bash
  sudo apt-get install libwebkit2gtk-4.1-dev libxdo-dev libssl-dev \
    libayatana-appindicator3-dev librsvg2-dev patchelf
  ```

Then:

```bash
git clone https://github.com/SaanPrasanna/Lumina.git
cd Lumina
corepack enable          # Windows without an admin shell: run pnpm as `corepack pnpm`
pnpm install
pnpm dev                 # the app with hot reload
```

`scripts/setup-dev.sh` and `scripts/setup-dev.ps1` do the `pnpm install` and
`cargo fetch` steps in one go.

Lumina allows only one running copy. Quit an installed Lumina before starting
the development build, or the second copy only brings the first to the front.

## Project layout

| You want to change… | Look in |
|---|---|
| How processes start and stop | `crates/lumina-process` |
| A service (PHP, nginx, MySQL, PostgreSQL…) | `crates/lumina-services/src/<service>` |
| Sites, hosts file and virtual hosts | `crates/lumina-vhost` |
| Certificates | `crates/lumina-ssl` |
| Downloads and the package catalog | `crates/lumina-packages`, `resources/catalog/packages.json` |
| Operating-system specifics | `crates/lumina-platform` |
| Config file schema and validation | `crates/lumina-config`, `resources/config/default.toml` |
| A command the UI calls | `apps/desktop/src-tauri/src/commands` |
| The tray and right-click menu | `apps/desktop/src-tauri/src/tray.rs` |
| A screen in the app | `apps/desktop/src/features/<feature>` |
| Generated server config files | `resources/templates` |
| Installers and releases | `packaging/`, `.github/workflows/release.yml` |

## Making a change

1. Fork the repository and branch from `dev`: `feat/<topic>`, `fix/<topic>`
   or `docs/<topic>`. `master` is the release branch: every push to it builds
   and publishes a release.
2. Keep each pull request to one topic. Unrelated clean-ups go in their own.
3. Write commit messages in the [Conventional Commits](https://www.conventionalcommits.org/)
   style: `feat(services): add PostgreSQL`, `fix(tray): …`, `docs: …`.
4. Add or update tests with the change (see below).
5. Add user-facing changes to the `[Unreleased]` section of
   [CHANGELOG.md](CHANGELOG.md), in plain words for users.
6. Open the pull request against `dev` and fill in the template.

### Code guidelines

- Match the code around you: naming, comment density and structure.
- Rust must pass `clippy` with warnings as errors. Return errors, do not
  `unwrap()` outside tests.
- Every process Lumina starts must also be stopped by Lumina. Changes to
  service start/stop code need a test showing no process is left behind.
- Downloads in the catalog need a `sha256` and a `checksum_source` saying where
  the checksum came from.
- UI text never goes straight into components: add a key to
  `apps/desktop/src/i18n/locales/en/*.json` and, if you can, to `si`. Product
  names (PHP, nginx, MySQL…) stay untranslated.
- Commands exposed to the UI are typed through tauri-specta. After changing
  one, regenerate the TypeScript bindings:
  ```bash
  LUMINA_UPDATE_BINDINGS=1 cargo test -p lumina-desktop --test bindings
  ```
- Workflow files must pass actionlint with shellcheck before they are pushed.

## Checks

Every pull request must pass:

```bash
cargo fmt --all --check
cargo lint                       # clippy, all targets, warnings as errors
cargo test --workspace
pnpm lint
pnpm typecheck
pnpm test
```

CI runs the same checks on Windows, macOS and Linux, plus `cargo deny` for
dependency licenses and advisories.

Tests that need real servers are skipped unless you point them at an install,
for example:

```bash
LUMINA_TEST_POSTGRES_DIR=/path/to/postgresql/18.6 \
  cargo test -p lumina-services --test db_real -- --test-threads=1
```

`crates/lumina-services/tests/db_real.rs` lists every variable.

## Review

A maintainer reviews every pull request. Expect questions; they are about the
code, not about you. A pull request is merged when the checks pass and review
comments are resolved. Maintainers may squash commits when merging.

## License of contributions

Lumina is licensed under the [GNU General Public License v3.0](LICENSE).
By submitting a contribution you agree that it is licensed under the same
license, and you confirm that you wrote it or otherwise have the right to
submit it under that license, as set out in the
[Developer Certificate of Origin](https://developercertificate.org/). Add a
`Signed-off-by` line to each commit to state this:

```bash
git commit -s -m "fix(sites): keep the hosts file line ending"
```

You keep the copyright in your contribution. Do not copy code from projects
whose license is incompatible with GPL-3.0.
