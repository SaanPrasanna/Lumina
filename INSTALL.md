# Installing Lumina

Download the file for your system from the
[latest release](https://github.com/SaanPrasanna/Lumina/releases/latest).

## Windows 10 and 11

| Your PC | File |
|---|---|
| Most PCs (Intel or AMD) | `Lumina_<version>_x64-setup.exe` |
| Windows on ARM (e.g. Snapdragon) | `Lumina_<version>_arm64-setup.exe` |
| Company deployment (per machine) | the `.msi` for your architecture |

1. Run the installer. The `.exe` installs for your user only and needs no
   administrator rights; the `.msi` installs for every user of the PC.
2. If the Microsoft Visual C++ runtime that PHP and Apache need is missing,
   the installer adds it (Windows asks for permission once).
3. Until the installers are code-signed, SmartScreen may show "Windows
   protected your PC": choose **More info → Run anyway**.

On Windows on ARM, Lumina runs natively; the servers it installs (PHP,
Apache, MySQL, …) are x64 builds, which Windows runs through its built-in
emulation.

Uninstalling (Settings → Apps) stops Lumina and its services first. The
`.exe` uninstaller asks whether to delete your data folder (projects,
databases, certificates); the answer defaults to **keep**. The `.msi` always
keeps it.

## macOS 11 or later

1. Open `Lumina_<version>_universal.dmg` (it runs on Apple silicon and Intel
   Macs) and drag **Lumina** to **Applications**.
2. Until the app is notarized, macOS blocks the first start: right-click
   Lumina in Applications, choose **Open**, then **Open** again.

## Linux

| Distribution | File | Install |
|---|---|---|
| Any | `Lumina_<version>_amd64.AppImage` | `chmod +x Lumina_*.AppImage && ./Lumina_*.AppImage` |
| Debian, Ubuntu, Mint | `Lumina_<version>_amd64.deb` | `sudo apt install ./Lumina_*_amd64.deb` |
| Fedora, openSUSE | `Lumina-<version>-1.x86_64.rpm` | `sudo dnf install ./Lumina-*.x86_64.rpm` |

ARM64 builds use `arm64` / `aarch64` in the file name.

Requirements: WebKitGTK 4.1 (Ubuntu 22.04+, Debian 12+, Fedora 36+). The
`.deb` and `.rpm` install it, `libnss3-tools` / `nss-tools` (to trust the
local HTTPS certificates in Firefox and Chrome) and a polkit rule so Lumina
can update `/etc/hosts` with your password. The AppImage needs WebKitGTK from
your distribution.

## First start

Lumina asks where to keep its data (default `C:\Lumina` on Windows,
`~/Lumina` elsewhere) and your language. Your projects go in the `www` folder
inside it; each folder becomes `http://<folder>.test`.

## Updates

Lumina checks for new versions once a day and offers them under
**Settings → About and updates**. Updates replace only the program; your data
folder is never touched.
