<div align="center">

<img src="assets/branding/app-icon.svg" alt="Lumina" width="96" height="96">

# Lumina

**A fast, open-source local development environment for web developers, on Windows, macOS and Linux.**

**🌐 [lumina.hexdev.tech](https://lumina.hexdev.tech/)**

[![Website](https://img.shields.io/badge/website-lumina.hexdev.tech-d97706)](https://lumina.hexdev.tech/)
[![Latest release](https://img.shields.io/github/v/release/SaanPrasanna/Lumina?sort=semver)](https://github.com/SaanPrasanna/Lumina/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/SaanPrasanna/Lumina/total)](https://github.com/SaanPrasanna/Lumina/releases)
[![CI](https://github.com/SaanPrasanna/Lumina/actions/workflows/ci.yml/badge.svg?branch=master)](https://github.com/SaanPrasanna/Lumina/actions/workflows/ci.yml)
[![License: GPL-3.0](https://img.shields.io/badge/license-GPL--3.0-blue.svg)](LICENSE)

[Website](https://lumina.hexdev.tech/) ·
[Download](https://github.com/SaanPrasanna/Lumina/releases/latest) ·
[Install guide](INSTALL.md) ·
[Changelog](CHANGELOG.md) ·
[Report a bug](https://github.com/SaanPrasanna/Lumina/issues/new/choose)

</div>

---

Lumina runs a complete web stack on your computer: PHP, a web server and
databases, each project at its own `https://<name>.test` address. Everything
is managed from one small app and the system tray, with no Docker and no
virtual machine. Every process Lumina starts stops with it, so nothing keeps
running in the background.

## Features

- **One-click stack.** PHP (FastCGI) with nginx or Apache, MySQL or MariaDB,
  PostgreSQL, Redis and Mailpit, plus MongoDB, Valkey, Meilisearch, Typesense
  and SeaweedFS (S3). Start All / Stop All from the dashboard or the tray.
- **Automatic sites.** Every folder in your projects folder becomes
  `<folder>.test`; the hosts file is kept up to date for you. Each site can
  use its own PHP version.
- **Node.js and Python too.** Vite, Next.js, Nuxt, Astro, SvelteKit, Django,
  FastAPI and Flask projects are served by their own dev server at
  `<folder>.test`, with hot reload and the runtime version the project asks for.
- **Trusted local HTTPS.** A certificate authority that exists only on your
  computer gives every site and `localhost` a certificate browsers trust.
- **Quick Add.** Install and switch versions of PHP, nginx, Apache, MySQL,
  MariaDB, PostgreSQL, Redis, the services above, Node.js, Python, Bun, Deno,
  uv, pnpm, Composer, Xdebug, phpMyAdmin, Adminer, ngrok and cloudflared, in
  several versions each (PHP back to 7.4, the three newest patches of a line).
  Every download is verified against its SHA-256 checksum before it is used.
- **QuickApp.** New Laravel, WordPress, Symfony, CodeIgniter, Drupal, Joomla,
  Statamic, Vite or plain PHP projects in one click, database and site included.
- **Profiles.** Save stacks such as "nginx · PHP 8.3 · MySQL 8.4" and switch
  between them in one click.
- **Tray control panel.** A menu per service to start or stop it, pick its
  version and open its config file or log. The same menu opens on right-click
  inside the window.
- **Terminal PATH.** `php`, `composer`, `node`, `bun`, `pnpm` and the rest in
  every terminal follow Lumina's active versions.
- **Sharing.** Put a site on a public HTTPS address through ngrok or
  Cloudflare Tunnel (no account needed), or share every site with phones on
  your Wi-Fi, with a QR code.
- **Debugging and testing.** Mailpit catches outgoing mail, Xdebug is one
  switch, each site lists its recent requests, a webhook catcher records and
  replays webhooks, and database snapshots restore a database in one click.
- **Logs.** Every service log in one place, live, with a filter.
- **Automatic updates.** Signed updates, verified before they are installed,
  on a stable or beta channel.
- **English and Sinhala.** The app, the tray and notifications in both languages.

## Installation

Download the installer for your system from the
[latest release](https://github.com/SaanPrasanna/Lumina/releases/latest):

| System | File |
|---|---|
| Windows 10/11 (x64 or ARM64) | `.exe` (per user) or `.msi` (per machine) |
| macOS 11 or later (Apple silicon and Intel) | `.dmg` |
| Linux x64 | `.AppImage`, `.deb` or `.rpm` |

[INSTALL.md](INSTALL.md) covers each system step by step, including first-start
warnings on unsigned builds, uninstalling, and where your data is kept.

## Getting started

1. Start Lumina and choose a data folder (default `C:\Lumina` on Windows,
   `~/Lumina` elsewhere).
2. Open **Quick Add** and install PHP, a web server and a database.
3. Press **Start All**.
4. Put a project in the `www` folder, or create one with **QuickApp**, and
   open `http://<folder>.test`.

## Building from source

Requirements:

- [Rust](https://rustup.rs/) (stable; the repository pins the toolchain)
- [Node.js](https://nodejs.org/) 22 or later, with Corepack for pnpm
- The [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for
  your system (WebView2 on Windows, Xcode Command Line Tools on macOS,
  WebKitGTK 4.1 and its companions on Linux)

```bash
git clone https://github.com/SaanPrasanna/Lumina.git
cd Lumina
corepack enable          # Windows without an admin shell: prefix pnpm with `corepack`
pnpm install
pnpm dev                 # the app with hot reload
pnpm build               # installers for this system
```

[CONTRIBUTING.md](CONTRIBUTING.md) explains the project layout, the checks
every change must pass, and how to submit a pull request.

## Project layout

```
apps/desktop/      The Tauri app: React UI (src/) and Rust shell (src-tauri/)
crates/            Rust crates: processes, services, sites, certificates, packages, …
resources/         Files bundled with the app: config templates, package catalog
packaging/         Installer configuration and release notes
xtask/             Build automation (`cargo xtask …`)
scripts/           Helper scripts for developers and CI
```

## Contributing

Bug reports, translations and pull requests are welcome. Please read
[CONTRIBUTING.md](CONTRIBUTING.md) first, and follow the
[Code of Conduct](CODE_OF_CONDUCT.md) in every project space.

## Security

Lumina edits the hosts file, installs a local certificate authority and
changes PATH, so security matters. Please report vulnerabilities privately as
described in [SECURITY.md](SECURITY.md), never in a public issue.

## License

Copyright (C) 2026 Sandun Prasanna

Lumina is free software: you can redistribute it and/or modify it under the
terms of the GNU General Public License as published by the Free Software
Foundation, version 3 of the License.

Lumina is distributed in the hope that it will be useful, but WITHOUT ANY
WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR
A PARTICULAR PURPOSE. See the [GNU General Public License](LICENSE) for more
details.

Every copy or modified version must keep this copyright notice and the
license, and must be distributed under the same license together with its
source code.

The servers and tools that Lumina downloads (PHP, nginx, MySQL, PostgreSQL and
others) are separate programs under their own licenses.
