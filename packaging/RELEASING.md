# Releasing Lumina

Installers are built by `.github/workflows/release.yml` when a version tag is
pushed. Everything below that needs an account or a secret is optional for a
test build; without it, that part is skipped and the result is unsigned.

## One-time setup

### Update signing (required for auto-updates)

Updates are installed only when they carry a valid signature for the public
key in `apps/desktop/src-tauri/tauri.conf.json` (`plugins.updater.pubkey`).

1. Keep the private key and its password (created with
   `pnpm tauri signer generate`) outside the repository, e.g. in a password
   manager. Losing them means existing installs can no longer be updated.
2. Add them as repository secrets:
   - `TAURI_SIGNING_PRIVATE_KEY`: the contents of the private key file
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: its password

### Windows code signing (no SmartScreen warning)

Azure Trusted Signing, or SignPath (free for open-source projects). For Azure
Trusted Signing, add:
`AZURE_CLIENT_ID`, `AZURE_CLIENT_SECRET`, `AZURE_TENANT_ID`,
`TRUSTED_SIGNING_ENDPOINT` (e.g. `https://weu.codesigning.azure.net`),
`TRUSTED_SIGNING_ACCOUNT`, `TRUSTED_SIGNING_PROFILE`.
The workflow then signs the app, `lumina-helper`, `lumina-shim` and the
installer.

### macOS signing and notarization (no Gatekeeper block)

Needs an Apple Developer account. Add:
`APPLE_CERTIFICATE` (base64 of the Developer ID Application .p12),
`APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`,
`APPLE_ID`, `APPLE_PASSWORD` (an app-specific password), `APPLE_TEAM_ID`.

## Every release

1. Set the version in `apps/desktop/src-tauri/tauri.conf.json`,
   `apps/desktop/src-tauri/Cargo.toml` and `apps/desktop/package.json`.
2. Merge to `master`, tag it and push the tag:
   `git tag v1.0.0 && git push origin v1.0.0`
   (a tag with a pre-release part, e.g. `v1.0.0-beta.1`, becomes a pre-release
   that only the beta channel offers).
3. The workflow creates a **draft** release with the installers and
   `latest.json`. Check it, then publish it.
4. Publishing runs `.github/workflows/update-channels.yml`, which copies
   `latest.json` to the rolling `beta` release. The stable channel reads the
   newest published non-pre-release directly.

## What the installers do

- Windows (NSIS, per user): installs the Visual C++ runtime (14.50+, needed by
  the VS18 builds of Apache and PHP) only when it is missing or older; the
  WebView2 bootstrapper is embedded. Uninstalling runs `lumina --quit` first
  and asks whether to also delete the data folder (default: keep; never asked
  during updates or silent uninstalls).
- macOS: universal binary, hardened runtime, entitlements from
  `packaging/macos/entitlements.plist`, DMG.
- Linux: `.deb` (needs `libnss3-tools`), `.rpm` (`nss-tools`) and AppImage;
  the deb/rpm install the polkit action for `lumina-helper`.
- `lumina-helper` and `lumina-shim` are bundled next to the app
  (`cargo xtask sidecars`, run by `beforeBuildCommand` in
  `tauri.release.conf.json`).

## Local test build (Windows)

```powershell
./scripts/fetch-vcredist.ps1
cd apps/desktop
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content -Raw $HOME\.tauri\lumina-updater.key
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = Get-Content -Raw $HOME\.tauri\lumina-updater.key.password
pnpm tauri build --config src-tauri/tauri.release.conf.json --config src-tauri/tauri.release.windows.conf.json --bundles nsis
```
