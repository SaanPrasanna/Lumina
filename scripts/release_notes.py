"""Writes the GitHub release notes for a version.

Usage: python scripts/release_notes.py <version> <repo> <assets.json> <checksums.txt> [<generated.md>]

- "What's new" comes from packaging/release-notes/<version>.md when it exists,
  otherwise from <generated.md> (GitHub's generated notes: merged changes).
- "Downloads" links the files the release actually holds, per system.
- assets.json: the release's assets as `gh release view --json assets` prints them.
- checksums.txt: `sha256sum` output for the installers.

Prints the Markdown to stdout.
"""

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# (row label, [(link text, file-name pattern)]). The first matching file wins.
DOWNLOADS = [
    ("**Windows 10/11** (x64)", [
        ("Installer (.exe)", r"_x64-setup\.exe$"),
        ("MSI", r"_x64_[a-zA-Z-]+\.msi$"),
    ]),
    ("**Windows 11 on ARM** (ARM64)", [
        ("Installer (.exe)", r"_arm64-setup\.exe$"),
        ("MSI", r"_arm64_[a-zA-Z-]+\.msi$"),
    ]),
    ("**macOS 11+** (Apple silicon and Intel)", [
        ("Disk image (.dmg)", r"_universal\.dmg$"),
    ]),
    ("**Linux** (x86_64)", [
        ("AppImage", r"_amd64\.AppImage$"),
        (".deb (Debian, Ubuntu)", r"_amd64\.deb$"),
        (".rpm (Fedora, openSUSE)", r"\.x86_64\.rpm$"),
    ]),
    ("**Linux** (ARM64)", [
        ("AppImage", r"_aarch64\.AppImage$"),
        (".deb", r"_arm64\.deb$"),
        (".rpm", r"\.aarch64\.rpm$"),
    ]),
]

# Update packages and signatures are for the updater, not for people.
NOT_FOR_PEOPLE = re.compile(r"(\.sig|\.tar\.gz|\.zip|latest\.json)$")


def whats_new(version: str, generated: str) -> str:
    notes = ROOT / "packaging" / "release-notes" / f"{version}.md"
    if notes.is_file():
        return notes.read_text(encoding="utf-8").strip()
    text = generated.strip()
    return text or "Maintenance release."


def downloads(names: list[str], base: str) -> str:
    rows = []
    for label, links in DOWNLOADS:
        cells = []
        for text, pattern in links:
            match = next((n for n in names if re.search(pattern, n)), None)
            if match:
                cells.append(f"[{text}]({base}/{match})")
        if cells:
            rows.append(f"| {label} | {' · '.join(cells)} |")
    if not rows:
        return "_The installers are still being built._"
    return "\n".join(["| System | Download |", "|---|---|", *rows])


def main() -> None:
    sys.stdout.reconfigure(encoding="utf-8")
    version, repo, assets_path, checksums_path = sys.argv[1:5]
    generated = Path(sys.argv[5]).read_text(encoding="utf-8") if len(sys.argv) > 5 else ""
    assets = json.loads(Path(assets_path).read_text(encoding="utf-8"))["assets"]
    names = sorted(a["name"] for a in assets if not NOT_FOR_PEOPLE.search(a["name"]))
    base = f"https://github.com/{repo}/releases/download/v{version}"
    checksums = Path(checksums_path).read_text(encoding="utf-8").strip()
    pre = "-" in version

    parts = [
        "## What's new",
        "",
        whats_new(version, generated),
        "",
        "## Downloads",
        "",
        downloads(names, base),
        "",
        "**Not sure which one?** Most Windows PCs need *Windows 10/11 (x64)*; "
        "every Mac uses the one *.dmg*; on Linux the *AppImage* runs on any distribution.",
        "",
        "> [!NOTE]",
        "> These builds are not code-signed yet. On Windows, SmartScreen may say "
        "\"Windows protected your PC\": choose **More info → Run anyway**. On macOS, "
        "right-click Lumina in Applications and choose **Open** the first time.",
        "",
        f"Installation steps and Linux requirements: [INSTALL.md](https://github.com/{repo}/blob/v{version}/INSTALL.md)",
    ]
    if pre:
        parts += [
            "",
            "> [!WARNING]",
            "> This is a pre-release. Installed copies on the **beta** update channel "
            "(Settings → About and updates) receive it automatically.",
        ]
    else:
        parts += [
            "",
            "Already using Lumina? It offers this version under **Settings → About and updates**.",
        ]
    if checksums:
        parts += [
            "",
            "<details>",
            "<summary>SHA-256 checksums</summary>",
            "",
            "```",
            checksums,
            "```",
            "",
            "</details>",
        ]
    print("\n".join(parts))


if __name__ == "__main__":
    main()
