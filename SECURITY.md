# Security Policy

Lumina changes sensitive parts of your system: it edits the hosts file (with
administrator approval), installs a certificate authority that your browsers
trust, puts its command folder on PATH, downloads and runs server software,
and installs updates of itself. Security reports are therefore handled with
priority.

## Supported versions

Security fixes are made for the latest release. Lumina updates itself, so
please make sure you are on the newest version before reporting.

| Version | Supported |
|---|---|
| Latest release | Yes |
| Older releases | No; update to the latest |

## Reporting a vulnerability

**Please do not report security problems in public issues, discussions or
pull requests.**

Report them privately, in one of these ways:

1. **GitHub private vulnerability reporting (preferred):**
   [open a private report](https://github.com/SaanPrasanna/Lumina/security/advisories/new).
2. **Email:** [sandunpmapa@gmail.com](mailto:sandunpmapa@gmail.com), with
   "Lumina security" in the subject.

Please include:

- the Lumina version and your operating system;
- what an attacker can do, and what they need first (for example, local access
  or a malicious website);
- steps to reproduce, or a proof of concept;
- any logs or screenshots that help.

## What happens next

| Step | Target time |
|---|---|
| Acknowledgement of your report | within 3 working days |
| First assessment (accepted or not, severity) | within 7 days |
| Fix released for confirmed critical or high issues | as soon as possible, normally within 30 days |

We keep you informed while we work on it, agree with you on when the details
are published, and credit you in the release notes and the security advisory
unless you prefer to stay anonymous.

## Scope

In scope:

- the Lumina app, its tray menu and its command-line options;
- its privileged helper (hosts file, certificate trust, PATH changes);
- the local certificate authority and the certificates it issues;
- package downloads and their checksum verification;
- the self-updater and the release pipeline;
- configuration that Lumina generates for the servers it runs.

Out of scope:

- vulnerabilities in the server software itself (PHP, nginx, Apache, MySQL,
  MariaDB, PostgreSQL, Redis, Node.js, Python, phpMyAdmin, ngrok): report
  them to those projects. Tell us if Lumina should stop offering an affected
  version;
- problems that require an attacker who already controls your user account
  or computer;
- the local development servers being reachable from your own computer, which
  is their purpose. Reaching them from another computer when they should not be
  is in scope.

## Security design

For context when assessing a report:

- Services listen on `127.0.0.1` only, unless you choose to share a site
  through ngrok.
- The certificate authority's private key never leaves your computer, and the
  authority is limited to issuing certificates for local development names.
- Every download in the package catalog is checked against a SHA-256 checksum
  before use.
- Updates are signed; the app refuses an update whose signature does not match.
- Administrator rights are requested only for specific actions (hosts file,
  certificate trust, system-wide PATH), each time you approve them.

Thank you for helping keep Lumina and its users safe.
