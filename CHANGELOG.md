# Changelog

All notable changes to this project are documented here.
Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning: [SemVer](https://semver.org/).

## [Unreleased]
- **License:** Lumina is now licensed under the GNU General Public License v3.0 (copyright Sandun Prasanna). Copies and modified versions must keep the copyright notice and stay open source under the same license.

## [0.2.0] - 2026-10-15
PostgreSQL joins the stack, the tray menu grows into a full control panel, and the app gets a new look.

- **PostgreSQL:** Install PostgreSQL 18 or 17 from Quick Add and run it next to MySQL or MariaDB. Its first start creates the data folder (user `postgres`, no password, local connections only); each major version keeps its own data. Start it with Start All, include it in profiles, change its port in Settings and edit `postgresql.conf` from its card. PHP loads the `pgsql` and `pdo_pgsql` extensions.
- **Tray menu:** One submenu per service, as in classic local-server panels: start or stop it, pick its version, open its config file and its log. A green dot marks what runs. www lists your projects, Tools opens a terminal, and Node.js and Python versions are a click away. If an action fails, a notification says why.
- **Right-click menu:** Right-clicking inside the window opens the same menu. Text fields keep their Cut, Copy and Paste menu.
- **New look:** A graphite dark theme and a clean light theme with an amber accent, the Manrope and Roboto Mono fonts, Noto Sans Sinhala for Sinhala text, and short, quiet animations (switched off when your system asks for reduced motion). Focused fields show an amber border.
- **Notepad++ by default:** New installs on Windows open projects and config files in Notepad++, found even when it is not on PATH. macOS and Linux keep VS Code. Choose any editor in Settings.
- **Fixes:** The tray follows a language change at once, and its version lists follow installs, settings and new projects.

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
