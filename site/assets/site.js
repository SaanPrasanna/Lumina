// Points every download link at the matching file of the latest release and
// offers the visitor's system first. Without JavaScript or the GitHub API,
// the links still lead to the latest release page.
(() => {
  const REPO = "SaanPrasanna/Lumina";

  // File name patterns of the release assets (see .github/workflows/release.yml).
  const ASSETS = {
    "win-x64": /_x64-setup\.exe$/,
    "win-arm64": /_arm64-setup\.exe$/,
    "win-x64-msi": /_x64_[^_]+\.msi$/,
    "win-arm64-msi": /_arm64_[^_]+\.msi$/,
    mac: /_universal\.dmg$/,
    "linux-appimage": /_amd64\.AppImage$/,
    "linux-deb": /_amd64\.deb$/,
    "linux-rpm": /\.x86_64\.rpm$/,
  };
  const PRIMARY = { windows: "win-x64", macos: "mac", linux: "linux-appimage" };

  const ua = navigator.userAgent;
  const os = /Windows/i.test(ua)
    ? "windows"
    : /Macintosh|Mac OS X/i.test(ua) && !/iPhone|iPad/i.test(ua)
      ? "macos"
      : /Linux|X11/i.test(ua) && !/Android/i.test(ua)
        ? "linux"
        : null;

  const hero = document.getElementById("hero-download");
  if (os && hero) {
    const label = hero.dataset[`label${os[0].toUpperCase()}${os.slice(1)}`];
    if (label) hero.querySelector("span").textContent = label;
    hero.dataset.asset = PRIMARY[os];
    document.querySelector(`.dl-card[data-os="${os}"]`)?.classList.add("current");
  }

  const formatSize = (bytes) => `${Math.round(bytes / 1048576)} MB`;

  fetch(`https://api.github.com/repos/${REPO}/releases/latest`, {
    headers: { Accept: "application/vnd.github+json" },
  })
    .then((r) => (r.ok ? r.json() : Promise.reject(new Error(String(r.status)))))
    .then((release) => {
      for (const el of document.querySelectorAll("[data-version]")) el.textContent = release.tag_name;
      for (const asset of release.assets ?? []) {
        for (const [key, pattern] of Object.entries(ASSETS)) {
          if (!pattern.test(asset.name)) continue;
          for (const link of document.querySelectorAll(`[data-asset="${key}"]`)) {
            link.href = asset.browser_download_url;
            link.title = `${asset.name} · ${formatSize(asset.size)}`;
          }
        }
      }
    })
    .catch(() => {
      // Rate-limited or offline: the links keep pointing at the release page.
    });
})();
