"""Renders the link preview images site/assets/og-en.png and og-si.png
(1200x630) with headless Chrome, using the site's own fonts and icon, so
Sinhala text is shaped correctly.

    python scripts/make_og.py [path/to/chrome]

Edit the texts below and run it again when the tagline changes.
"""
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile

SITE = pathlib.Path(__file__).resolve().parent.parent / "site"
OUT = pathlib.Path(tempfile.mkdtemp(prefix="lumina-og-"))
CHROME = next((c for c in [
    *sys.argv[1:],
    r"C:\Program Files\Google\Chrome\Application\chrome.exe",
    shutil.which("google-chrome"), shutil.which("chromium"), shutil.which("chrome"),
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
] if c and os.path.exists(c)), None)
if not CHROME:
    sys.exit("Chrome not found; pass its path as the first argument.")
assets = SITE.joinpath("assets").as_uri()

TEXTS = {
    "en": ("Local development,", "one click away", "Free and open source · Windows, macOS and Linux"),
    "si": ("ඔබේ සංවර්ධන පරිසරය,", "එක ක්ලික් එකකින්", "නොමිලේ සහ විවෘත මූලාශ්‍ර · Windows, macOS සහ Linux"),
}
CHIPS = ["PHP", "Node.js", "Python", "MySQL", "PostgreSQL", "Redis"]

for lang, (line1, line2, sub) in TEXTS.items():
    chips = "".join(f"<span>{c}</span>" for c in CHIPS)
    page = f"""<!doctype html><html lang="{lang}"><head><meta charset="utf-8"><style>
@font-face {{ font-family: Manrope; src: url("{assets}/fonts/manrope-latin.woff2") format("woff2"); font-weight: 200 800; }}
@font-face {{ font-family: "Noto Sans Sinhala"; src: url("{assets}/fonts/noto-sans-sinhala.woff2") format("woff2"); font-weight: 100 900; }}
html, body {{ margin: 0; width: 1200px; height: 630px; overflow: hidden; }}
body {{ position: relative; background: #1b1e20; color: #eae7e2; font-family: Manrope, "Noto Sans Sinhala", sans-serif; }}
body::before {{ content: ""; position: absolute; inset: 0;
  background: radial-gradient(circle at 82% 18%, rgb(216 164 76 / 0.22), transparent 42%); }}
.box {{ position: absolute; left: 92px; top: 78px; right: 80px; }}
.brand {{ display: flex; align-items: center; gap: 42px; }}
.brand img {{ width: 112px; height: 112px; }}
.brand b {{ font-size: 96px; font-weight: 700; letter-spacing: -0.02em; }}
h1 {{ margin: 50px 0 0; font-size: {64 if lang == "en" else 56}px; line-height: 1.18; font-weight: 800; letter-spacing: -0.01em; }}
h1 span {{ color: #d8a44c; }}
p {{ margin: 22px 0 0; font-size: 30px; color: #b6b9bd; }}
.chips {{ display: flex; gap: 14px; margin-top: 36px; }}
.chips span {{ padding: 8px 22px; border: 2px solid #2f3337; border-radius: 999px; font-size: 24px; font-weight: 700; color: #d7d4cf; }}
.bar {{ position: absolute; left: 0; right: 0; bottom: 0; height: 10px; background: #d8a44c; }}
</style></head><body>
<div class="box">
  <div class="brand"><img src="{assets}/icon.svg" alt=""><b>Lumina</b></div>
  <h1>{line1}<br><span>{line2}</span></h1>
  <p>{sub}</p>
  <div class="chips">{chips}</div>
</div>
<div class="bar"></div>
</body></html>"""
    html_file = OUT / f"og-{lang}.html"
    html_file.write_text(page, encoding="utf-8")
    png = SITE / "assets" / f"og-{lang}.png"
    subprocess.run([CHROME, "--headless=new", "--disable-gpu", "--hide-scrollbars", "--force-device-scale-factor=1",
                    "--allow-file-access-from-files", "--virtual-time-budget=3000",
                    f"--screenshot={png}", "--window-size=1200,630", html_file.as_uri()], check=True,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    print("wrote", png)
