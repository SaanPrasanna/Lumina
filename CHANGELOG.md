# Changelog

All notable changes to this project are documented here.
Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning: [SemVer](https://semver.org/).

## [Unreleased]

## [0.4.0] - 2026-10-18
Lumina grows beyond PHP: Node.js and Python projects get their own `.test` address, more databases and services join the stack, and sharing, debugging and testing tools come built in.

- **Dev servers behind .test:** Node.js and Python projects get their own address too. A folder with Vite, Next.js, Nuxt, Astro, SvelteKit or React Router (and no PHP), or with Django, FastAPI or Flask, is served by its own dev server at `https://<folder>.test`, with hot reload. Start it from the Sites page; Lumina runs it with its own Node.js and Python, on a fixed port per site, and shows its log. Any other command works too (Edit › Served by), and a dev server can start with Start All.
- **More runtimes:** Bun, Deno, uv and pnpm in Quick Add, each on PATH in every terminal (`bun`/`bunx`, `deno`, `uv`/`uvx`, `pnpm`/`pnpx`). A project's `.nvmrc`, `.node-version`, `.python-version` or `.tool-versions` chooses the Node.js or Python its dev server runs with.
- **A PHP version per site:** Edit a site and pick its PHP version; it gets its own PHP pool while other sites keep the shared one. Works with nginx and Apache.
- **More services:** MongoDB, Valkey, Meilisearch, Typesense and SeaweedFS (S3-compatible storage) from Quick Add, each with its own card on the Services page and a place in Start All. Adminer, a one-file database manager for MySQL, MariaDB, PostgreSQL and SQLite, opens at `http://localhost/adminer/`.
- **Mailpit:** Install Mailpit from Quick Add and every mail your sites send lands in a local inbox instead of a real mailbox. Apps send to `127.0.0.1:1025` with any login; the inbox opens from its card on the Services page or from the tray. PHP's `mail()` goes there too (switch it off in Settings › Mail), and new Laravel projects from QuickApp are set up to use it. Start it with Start All like Redis.
- **Debugging:** Xdebug in one switch on the PHP card (Windows builds for PHP 8.1–8.5 in Quick Add), with a VS Code debug setup written into the project on request. Each site lists its latest requests (method, path, status, time). A webhook catcher records webhooks on a local address, can pass them on to a site, and replays any of them later.
- **Testing:** Database snapshots for MySQL, MariaDB and PostgreSQL: save a database before a migration and restore it in one click. Install Playwright's browsers for a project from its row on the Sites page. Dev servers trust Lumina's certificates, so their requests to `https://*.test` work.
- **Sharing:** Share sites through Cloudflare Tunnel with no account (a random `trycloudflare.com` address), next to ngrok. "Share on my network" serves the sites to phones and tablets on the same Wi-Fi at `http://<site>.<ip>.nip.io`, with a QR code; phpMyAdmin and Adminer still answer only this computer.
- **Quick Add:** More versions to choose from (three of most tools and services, the long-term MongoDB lines among them), and every download shows its size. A paused download can be discarded instead of only resumed. Tools with no download for your system (Valkey and Typesense on Windows) are no longer listed there.
- **Not installed, said plainly:** Services that are not installed carry a "Not installed" label on the Dashboard and the Services page, with an Install button that opens Quick Add on them, instead of a Start button that fails. Starting one anyway says "MySQL is not installed yet" with the same button, and the service is no longer marked as failed.
- **Settings, redesigned:** Every setting sits on its own row with a short explanation, on/off options are switches, and a section's Save button wakes up only when something changed, with Undo next to it. A side index follows you down the page, and the page uses the whole window. About and updates, now last, shows the license and links to the source code.
- **Cleaner window:** The sidebar starts straight with the pages; the window title already names the app.
- **Sleep and resume:** When the computer wakes up, Lumina checks that every running service still answers and restarts any that does not, renews certificates that came due while it slept, and refreshes the Sharing page.
- **Fixes:** Stopping a terminal tool that runs `node`, `php` or `python` through Lumina (for example a Vite dev server) now stops the program too, instead of leaving it running and holding its port; Lumina updates those commands even while one of them is running. "Open terminal" works with Windows Terminal again, instead of opening a tab that fails with `%PATH%`. Choosing a version is no longer offered for Adminer and Xdebug, where it failed.

## [0.3.0] - 2026-10-15
Lumina now sets up a stack on macOS and Linux, using official downloads and Homebrew, the standard way on those systems.

- **macOS and Linux downloads:** Quick Add installs MySQL (macOS and Linux), PostgreSQL (macOS) and Python (macOS and Linux) straight from their official downloads, each checked against its SHA-256.
- **Homebrew:** PHP, nginx, Apache, MariaDB and Redis publish no downloads for macOS and Linux. Quick Add shows the `brew install` command for them, and Lumina finds the Homebrew versions on its own and runs them with its own settings, sites and HTTPS. Homebrew versions carry a Homebrew badge and can be chosen like any other.
- **Clearer messages:** Quick Add explains how to get a server instead of only saying that no download exists.
- **Fixes:** On macOS the window no longer scrolls or bounces past its edges. Quick Add unpacks `.tar.xz` archives (MySQL for Linux).

## [0.2.0] - 2026-10-15
PostgreSQL joins the stack, the tray menu grows into a full control panel, and the app gets a new look.

- **PostgreSQL:** Install PostgreSQL 18 or 17 from Quick Add and run it next to MySQL or MariaDB. Its first start creates the data folder (user `postgres`, no password, local connections only); each major version keeps its own data. Start it with Start All, include it in profiles, change its port in Settings and edit `postgresql.conf` from its card. PHP loads the `pgsql` and `pdo_pgsql` extensions.
- **Tray menu:** One submenu per service, as in classic local-server panels: start or stop it, pick its version, open its config file and its log. A green dot marks what runs. www lists your projects, Tools opens a terminal, and Node.js and Python versions are a click away. If an action fails, a notification says why.
- **Right-click menu:** Right-clicking inside the window opens the same menu. Text fields keep their Cut, Copy and Paste menu.
- **New look:** A graphite dark theme and a clean light theme with an amber accent, the Manrope and Roboto Mono fonts, Noto Sans Sinhala for Sinhala text, and short, quiet animations (switched off when your system asks for reduced motion). Focused fields show an amber border.
- **Notepad++ by default:** New installs on Windows open projects and config files in Notepad++, found even when it is not on PATH. macOS and Linux keep VS Code. Choose any editor in Settings.
- **Fixes:** The tray follows a language change at once, and its version lists follow installs, settings and new projects.
- **License:** Lumina is now licensed under the GNU General Public License v3.0 (copyright Sandun Prasanna). Copies and modified versions must keep the copyright notice and stay open source under the same license.

## [0.1.0] - 2026-10-15
The first release of Lumina: a local development environment for PHP for Windows, macOS and Linux.

- **One-click stack:** PHP (FastCGI) with nginx or Apache, MySQL or MariaDB, and Redis. Start All / Stop All from the dashboard or the tray; every process Lumina starts is stopped with it, so nothing keeps running in the background.
- **Automatic sites:** Every folder in your projects folder becomes `<folder>.test`, with the hosts file kept up to date (one administrator prompt when it changes).
- **HTTPS that just works:** A certificate authority that exists only on your computer gives every site and `localhost` a trusted certificate. Force HTTPS is one switch.
- **Quick Add:** Install and switch PHP, nginx, Apache, MySQL, MariaDB, Redis, Node.js, Python, Composer, phpMyAdmin and ngrok versions. Every download is checked against its SHA-256 before it is used.
- **QuickApp:** New Laravel, WordPress, Symfony, CodeIgniter, Drupal, Joomla, Statamic, Vite or plain PHP projects in one click, with their database created and their site ready. Cancelling stops everything the installer started.
- **Profiles:** Save stacks such as "nginx · PHP 8.3 · MySQL 8.4" and switch between them in one click, from the app or the tray.
- **Terminal PATH:** `php`, `composer` and `node` in every terminal follow Lumina's active versions.
- **Sharing:** Share a site on a public HTTPS address through ngrok, with a QR code for your phone. Links and redirects use the public address.
- **Logs:** Every log in one place, live, with a filter.
- **English and Sinhala:** The whole app, the tray menu and notifications in both languages.
- **Automatic updates:** Lumina checks for new versions and installs them on request. Updates are signed and verified first, and the services that were running start again afterwards. Choose the stable or beta channel in Settings.
