"""Generates the website pages from the texts below, so both languages keep
the same layout: site/index.html (English), site/si/index.html (Sinhala),
site/changelog/ (every release from CHANGELOG.md, with an Atom feed) and
site/404.html, plus the sitemaps (sitemap.xml lists page-sitemap.xml) and
site/sitemap.xsl, which shows them as a readable page in a browser.

    python scripts/build_site.py

Edit the texts or the templates here, run the script and commit the result.
The styles are in site/assets/style.css, the download links in site/assets/site.js,
the link preview images come from scripts/make_og.py.
"""
import html
import json
import os
import re
import subprocess
from datetime import datetime, timezone

ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'site')) + os.sep
BASE = 'https://lumina.hexdev.tech/'
REPO = 'https://github.com/SaanPrasanna/Lumina'
LATEST = REPO + '/releases/latest'
AUTHOR = 'Sandun Prasanna'
AUTHOR_URL = 'https://github.com/SaanPrasanna'
PUBLISHED = '2026-10-15'  # the website's first release

ICONS = {
    'stack': '<path d="M12 3 3 8l9 5 9-5-9-5Z"/><path d="m3 13 9 5 9-5"/>',
    'globe': '<circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18"/>',
    'lock': '<rect x="5" y="11" width="14" height="10" rx="2"/><path d="M8 11V7a4 4 0 0 1 8 0v4"/>',
    'package': '<path d="m12 3 8 4.5v9L12 21l-8-4.5v-9L12 3Z"/><path d="m4 7.5 8 4.5 8-4.5M12 12v9"/>',
    'rocket': '<path d="M5 15c-1.5 1.5-2 5-2 5s3.5-.5 5-2"/><path d="M9 15 6 12c1-4 5-9 12-9 0 7-5 11-9 12Z"/><circle cx="14.5" cy="9.5" r="1.5"/>',
    'layers': '<path d="m12 3 9 5-9 5-9-5 9-5Z"/><path d="m3 12 9 5 9-5M3 16l9 5 9-5"/>',
    'menu': '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M7 9h10M7 13h10M7 17h6"/>',
    'share': '<circle cx="6" cy="12" r="2.5"/><circle cx="18" cy="6" r="2.5"/><circle cx="18" cy="18" r="2.5"/><path d="m8.2 10.8 7.6-3.6M8.2 13.2l7.6 3.6"/>',
    'terminal': '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="m7 9 3 3-3 3M13 15h4"/>',
    'refresh': '<path d="M20 11a8 8 0 0 0-14.9-3M4 13a8 8 0 0 0 14.9 3"/><path d="M4 4v4h4M20 20v-4h-4"/>',
    'windows': '<path d="M3 5.5 10.5 4.5v7H3zM12 4.3 21 3v8.5h-9zM3 12.5h7.5v7L3 18.5zM12 12.5h9V21l-9-1.3z" fill="currentColor" stroke="none"/>',
    'apple': '<path d="M16.4 12.6c0-2.4 2-3.5 2-3.6-1.1-1.6-2.8-1.8-3.4-1.8-1.4-.1-2.8.9-3.5.9s-1.8-.8-3-.8C6.9 7.3 5 8.6 5 11.6c0 1.9.7 3.8 1.6 5.1.8 1.1 1.5 2 2.5 2s1.4-.6 2.7-.6 1.6.6 2.7.6 1.8-1 2.5-2c.5-.7.9-1.5 1.1-2.2-1.5-.6-1.7-1.9-1.7-1.9ZM14.3 5.8c.6-.7 1-1.7.9-2.7-.9.1-1.9.6-2.5 1.3-.5.6-1 1.6-.9 2.6 1 .1 1.9-.5 2.5-1.2Z" fill="currentColor" stroke="none"/>',
    'linux': '<path d="M12 3c-2 0-3 1.7-3 4 0 1.5-.4 2.6-1.4 4C6.4 12.7 5 14.6 5 17c0 1.6 1 2.5 2.4 2.8L9 21h6l1.6-1.2C18 19.5 19 18.6 19 17c0-2.4-1.4-4.3-2.6-6-1-1.4-1.4-2.5-1.4-4 0-2.3-1-4-3-4Z"/><circle cx="10.5" cy="7.5" r=".6" fill="currentColor"/><circle cx="13.5" cy="7.5" r=".6" fill="currentColor"/>',
    'github': '<path d="M12 2a10 10 0 0 0-3.2 19.5c.5.1.7-.2.7-.5v-1.7c-2.8.6-3.4-1.3-3.4-1.3-.5-1.2-1.1-1.5-1.1-1.5-.9-.6.1-.6.1-.6 1 .1 1.5 1 1.5 1 .9 1.5 2.4 1.1 2.9.8.1-.7.4-1.1.7-1.3-2.2-.3-4.6-1.1-4.6-5 0-1.1.4-2 1-2.7-.1-.3-.4-1.3.1-2.7 0 0 .8-.3 2.8 1a9.6 9.6 0 0 1 5 0c1.9-1.3 2.8-1 2.8-1 .5 1.4.2 2.4.1 2.7.6.7 1 1.6 1 2.7 0 3.9-2.4 4.7-4.6 5 .4.3.7.9.7 1.9v2.8c0 .3.2.6.7.5A10 10 0 0 0 12 2Z" fill="currentColor" stroke="none"/>',
    'download': '<path d="M12 4v11M7 10l5 5 5-5M5 20h14"/>',
    'bug': '<rect x="7" y="7" width="10" height="13" rx="5"/><path d="M12 11v9M7 12H3M21 12h-4M7 17l-3 2M17 17l3 2M8 8 5 5M16 8l3-3M9 7a3 3 0 0 1 6 0"/>',
    'rss': '<path d="M5 11a8 8 0 0 1 8 8M5 5a14 14 0 0 1 14 14"/><circle cx="6" cy="18" r="1.5" fill="currentColor"/>',
}


def icon(name, size=22):
    return (f'<svg class="i" width="{size}" height="{size}" viewBox="0 0 24 24" fill="none" '
            f'stroke="currentColor" stroke-width="1.8" stroke-linecap="round" '
            f'stroke-linejoin="round" aria-hidden="true">{ICONS[name]}</svg>')


STACK = ['PHP', 'Node.js', 'Python', 'Bun', 'Deno', 'nginx', 'Apache', 'MySQL', 'MariaDB',
         'PostgreSQL', 'MongoDB', 'Redis', 'Valkey', 'Meilisearch', 'Mailpit', 'Composer', 'Xdebug']

T = {
    'en': {
        'lang': 'en', 'dir': '', 'path': '', 'og': 'og-en.png',
        'title': 'Lumina – free local development environment for Windows, macOS and Linux',
        'description': ('Lumina is a free, open-source local development environment for web developers. '
                        'Run PHP, Node.js and Python with nginx or Apache, MySQL, PostgreSQL, Redis and more, '
                        'with automatic .test sites and trusted HTTPS, on Windows, macOS and Linux.'),
        'og_title': 'Lumina – local development, one click away',
        'og_alt': 'Lumina – local development, one click away. Free and open source for Windows, macOS and Linux, '
                  'with PHP, Node.js, Python, MySQL and PostgreSQL.',
        'app_kind': 'Local development environment',
        'nav_features': 'Features', 'nav_download': 'Download', 'nav_faq': 'FAQ', 'nav_changelog': 'Changelog',
        'switch_label': 'සිංහල', 'switch_title': 'සිංහලෙන් කියවන්න',
        'skip': 'Skip to content',
        'badge': 'Free and open source',
        'h1': 'Your dev stack,<br><span class="hl">one click away</span>',
        'lead': ('Lumina runs PHP, Node.js, Python, web servers and databases on your own computer. '
                 'Every project gets its own <code>https://name.test</code> address. '
                 'No Docker, no virtual machine, nothing left running when you quit.'),
        'dl_generic': 'Download Lumina', 'dl_windows': 'Download for Windows',
        'dl_macos': 'Download for macOS', 'dl_linux': 'Download for Linux',
        'github_btn': 'View on GitHub',
        'meta_line': 'Windows · macOS · Linux · <span data-version>latest</span>',
        'stack_title': 'Everything a web project needs',
        'features_title': 'Built for everyday development',
        'features_lead': 'Fast to start, quiet in the background, and out of your way.',
        'features': [
            ('stack', 'One-click stack', 'nginx or Apache, MySQL or MariaDB, PostgreSQL, Redis, MongoDB, Meilisearch and more. Start All and Stop All from the app or the tray menu.'),
            ('globe', 'Automatic sites', 'Every folder becomes <code>folder.test</code>: PHP sites with their own PHP version, and Vite, Next.js, Nuxt, Django or FastAPI projects behind their dev server, with hot reload.'),
            ('lock', 'Trusted local HTTPS', 'A certificate authority that exists only on your computer gives every site a padlock your browsers trust.'),
            ('package', 'Quick Add', 'Install and switch versions of PHP, Node.js, Python, Bun, Deno, servers and databases. Every download is checked against its SHA-256. On macOS and Linux, PHP and the web servers come from Homebrew.'),
            ('rocket', 'QuickApp', 'Laravel, WordPress, Symfony, Drupal, Joomla and more in one click, with the database created and the site ready.'),
            ('bug', 'Debug and test', 'Mailpit catches outgoing mail, Xdebug is one switch, each site lists its requests, a webhook catcher replays webhooks and database snapshots restore in one click.'),
            ('share', 'Share a site', 'A public HTTPS address through Cloudflare Tunnel (no account) or ngrok, or every site on phones on your Wi-Fi, with a QR code.'),
            ('layers', 'Profiles', 'Save stacks like “nginx · PHP 8.3 · MySQL 8.4” and switch between them in one click.'),
            ('refresh', 'Signed updates', 'Lumina updates itself. Every update is signed and verified first, and your services start again afterwards.'),
        ],
        'how_title': 'Up and running in minutes',
        'steps': [
            ('Install Lumina', 'Download the installer for your system and run it. No administrator rights needed on Windows.'),
            ('Add your tools', 'Open Quick Add and install PHP or Node.js, a web server and a database, in the versions you need.'),
            ('Start building', 'Press Start All, put a project in the <code>www</code> folder and open <code>http://project.test</code>.'),
        ],
        'download_title': 'Download',
        'download_lead': 'Lumina is free. Pick the file for your system; the newest version is always here.',
        'win_title': 'Windows 10 and 11',
        'win_items': [('win-x64', 'Installer for most PCs (x64)'), ('win-arm64', 'Windows on ARM (ARM64)'),
                      ('win-x64-msi', 'MSI for company deployment (x64)'), ('win-arm64-msi', 'MSI (ARM64)')],
        'mac_title': 'macOS 11 or later',
        'mac_items': [('mac', 'Apple silicon and Intel (.dmg)')],
        'linux_title': 'Linux (x64)',
        'linux_items': [('linux-appimage', 'AppImage (any distribution)'), ('linux-deb', 'Debian, Ubuntu (.deb)'),
                        ('linux-rpm', 'Fedora, openSUSE (.rpm)')],
        'install_guide': 'Step-by-step install guide',
        'all_releases': 'All releases and checksums',
        'faq_title': 'Questions',
        'faq': [
            ('Is Lumina free?', 'Yes. Lumina is open source under the GNU General Public License v3.0. There is no paid edition.'),
            ('Do I need Docker or a virtual machine?', 'No. Lumina runs the real PHP, Node.js, web server and database programs directly on your computer, so it starts fast and uses little memory.'),
            ('Does it work for Node.js and Python projects?', 'Yes. A folder with Vite, Next.js, Nuxt, Astro, SvelteKit, Django, FastAPI or Flask is served by its own dev server at <code>https://folder.test</code>, with the Node.js or Python version the project asks for in <code>.nvmrc</code> or <code>.python-version</code>.'),
            ('Which PHP versions can I use?', 'On Windows, every version in Quick Add, side by side. On macOS and Linux, the versions you install with Homebrew (for example <code>brew install php</code> or <code>php@8.3</code>), which Lumina finds on its own. Switch with one click, per stack or per profile.'),
            ('Can I use PostgreSQL and MySQL together?', 'Yes. PostgreSQL runs next to MySQL or MariaDB, each on its own port.'),
            ('Where are my projects and databases kept?', 'In one data folder you choose on first start (<code>C:\\Lumina</code> on Windows, <code>~/Lumina</code> elsewhere). Uninstalling keeps it unless you ask otherwise.'),
            ('Is it safe?', 'Services listen only on your own computer. Downloads are verified, updates are signed, and administrator rights are asked for only when they are needed.'),
        ],
        'footer_tag': 'A local development environment for web developers.',
        'footer_license': 'Released under the <a href="https://github.com/SaanPrasanna/Lumina/blob/master/LICENSE">GNU GPL v3.0</a>.',
        'footer_links': [('GitHub', REPO), ('Releases', REPO + '/releases'), ('Changelog', BASE + 'changelog/'),
                         ('Report a bug', REPO + '/issues/new/choose'), ('Security', REPO + '/blob/master/SECURITY.md')],
        'mock_running': 'Running', 'mock_stopped': 'Stopped', 'mock_start_all': 'Stop All',
        'mock_services': 'Services',
    },
    'si': {
        'lang': 'si', 'dir': '', 'path': 'si/', 'og': 'og-si.png',
        'title': 'Lumina – Windows, macOS සහ Linux සඳහා නොමිලේ දේශීය සංවර්ධන පරිසරය',
        'description': ('Lumina යනු වෙබ් සංවර්ධකයින් සඳහා නොමිලේ ලබා ගත හැකි විවෘත මූලාශ්‍ර දේශීය සංවර්ධන පරිසරයකි. '
                        'nginx හෝ Apache, MySQL, PostgreSQL, Redis සහ තවත් දේ සමඟ PHP, Node.js සහ Python ධාවනය කරන්න; ස්වයංක්‍රීය '
                        '.test අඩවි සහ විශ්වාසනීය HTTPS සමඟ, Windows, macOS සහ Linux මත.'),
        'og_title': 'Lumina – ඔබේ සංවර්ධන පරිසරය, එක ක්ලික් එකකින්',
        'og_alt': 'Lumina – ඔබේ සංවර්ධන පරිසරය, එක ක්ලික් එකකින්. Windows, macOS සහ Linux සඳහා නොමිලේ සහ විවෘත මූලාශ්‍ර; '
                  'PHP, Node.js, Python, MySQL සහ PostgreSQL සමඟ.',
        'app_kind': 'දේශීය සංවර්ධන පරිසරය',
        'nav_features': 'විශේෂාංග', 'nav_download': 'බාගන්න', 'nav_faq': 'ප්‍රශ්න', 'nav_changelog': 'වෙනස්කම්',
        'switch_label': 'English', 'switch_title': 'Read in English',
        'skip': 'අන්තර්ගතයට යන්න',
        'badge': 'නොමිලේ සහ විවෘත මූලාශ්‍ර',
        'h1': 'ඔබේ සංවර්ධන පරිසරය,<br><span class="hl">එක ක්ලික් එකකින්</span>',
        'lead': ('Lumina ඔබේම පරිගණකයේ PHP, Node.js, Python, වෙබ් සේවාදායක සහ ඔබේ දත්ත සමුදා ධාවනය කරයි. '
                 'සෑම ව්‍යාපෘතියකටම තමන්ගේම <code>https://name.test</code> ලිපිනයක් ලැබේ. '
                 'Docker නැත, අතථ්‍ය යන්ත්‍ර නැත; ඔබ ඉවත් වූ විට කිසිවක් ධාවනය වෙමින් නොපවතී.'),
        'dl_generic': 'Lumina බාගන්න', 'dl_windows': 'Windows සඳහා බාගන්න',
        'dl_macos': 'macOS සඳහා බාගන්න', 'dl_linux': 'Linux සඳහා බාගන්න',
        'github_btn': 'GitHub හි බලන්න',
        'meta_line': 'Windows · macOS · Linux · <span data-version>නවතම</span>',
        'stack_title': 'වෙබ් ව්‍යාපෘතියකට අවශ්‍ය සියල්ල',
        'features_title': 'දිනපතා සංවර්ධනය සඳහා',
        'features_lead': 'ඉක්මනින් ආරම්භ වේ, පසුබිමේ නිශ්ශබ්දව ක්‍රියා කරයි, ඔබට බාධා නොකරයි.',
        'features': [
            ('stack', 'එක් ක්ලික් පරිසරය', 'nginx හෝ Apache, MySQL හෝ MariaDB, PostgreSQL, Redis, MongoDB, Meilisearch සහ තවත් දේ. යෙදුමෙන් හෝ පද්ධති තැටි මෙනුවෙන් සියල්ල ආරම්භ කරන්න, නවත්වන්න.'),
            ('globe', 'ස්වයංක්‍රීය අඩවි', 'සෑම ෆෝල්ඩරයක්ම <code>folder.test</code> බවට පත් වේ: තමන්ගේම PHP සංස්කරණය සහිත PHP අඩවි, සහ hot reload සමඟ තමන්ගේ dev සේවාදායකය පිටුපස Vite, Next.js, Nuxt, Django හෝ FastAPI ව්‍යාපෘති.'),
            ('lock', 'විශ්වාසනීය දේශීය HTTPS', 'ඔබේ පරිගණකයේ පමණක් පවතින සහතික අධිකාරියක් සෑම අඩවියකටම බ්‍රව්සර විශ්වාස කරන ආරක්ෂිත අගුලක් ලබා දෙයි.'),
            ('package', 'ඉක්මන් එකතුව', 'PHP, Node.js, Python, Bun, Deno, සේවාදායක සහ දත්ත සමුදා සංස්කරණ ස්ථාපනය කර මාරු කරන්න. සෑම බාගැනීමක්ම SHA-256 මගින් පරීක්ෂා කෙරේ. macOS සහ Linux මත PHP සහ වෙබ් සේවාදායක Homebrew මගින් ලැබේ.'),
            ('rocket', 'QuickApp', 'Laravel, WordPress, Symfony, Drupal, Joomla සහ තවත් බොහෝ දේ එක ක්ලික් එකකින්; දත්ත සමුදාය සාදා, අඩවිය සූදානම්.'),
            ('bug', 'දෝෂ නිරාකරණය සහ පරීක්ෂාව', 'Mailpit පිටතට යන ඊමේල් අල්ලා ගනී, Xdebug එක ස්විචයකි, සෑම අඩවියක්ම එහි ඉල්ලීම් පෙන්වයි, webhook ග්‍රාහකයක් webhooks නැවත යවයි, දත්ත සමුදා snapshots එක ක්ලික් එකකින් ප්‍රතිස්ථාපනය වේ.'),
            ('share', 'අඩවියක් බෙදාගන්න', 'Cloudflare Tunnel (ගිණුමක් නැතිව) හෝ ngrok හරහා පොදු HTTPS ලිපිනයක්, නැතහොත් ඔබේ Wi-Fi හි දුරකථනවලට සියලු අඩවි; QR කේතයක් ද සමඟ.'),
            ('layers', 'පැතිකඩ', '“nginx · PHP 8.3 · MySQL 8.4” වැනි පරිසර සුරකින්න, එක ක්ලික් එකකින් ඒවා අතර මාරු වන්න.'),
            ('refresh', 'අත්සන් කළ යාවත්කාලීන', 'Lumina තමන්ම යාවත්කාලීන වේ. සෑම යාවත්කාලීනයක්ම අත්සන් කර මුලින්ම තහවුරු කෙරේ; පසුව ඔබේ සේවා නැවත ආරම්භ වේ.'),
        ],
        'how_title': 'මිනිත්තු කිහිපයකින් ක්‍රියාත්මකයි',
        'steps': [
            ('Lumina ස්ථාපනය කරන්න', 'ඔබේ පද්ධතියට ගැළපෙන ස්ථාපකය බාගෙන ධාවනය කරන්න. Windows මත පරිපාලක අවසර අවශ්‍ය නැත.'),
            ('මෙවලම් එකතු කරන්න', 'ඉක්මන් එකතුව විවෘත කර ඔබට අවශ්‍ය සංස්කරණවලින් PHP හෝ Node.js, වෙබ් සේවාදායකයක් සහ දත්ත සමුදායක් ස්ථාපනය කරන්න.'),
            ('ගොඩනැගීම අරඹන්න', 'සියල්ල ආරම්භ කරන්න ඔබා, ව්‍යාපෘතියක් <code>www</code> ෆෝල්ඩරයට දමා <code>http://project.test</code> විවෘත කරන්න.'),
        ],
        'download_title': 'බාගන්න',
        'download_lead': 'Lumina නොමිලේ. ඔබේ පද්ධතියට ගැළපෙන ගොනුව තෝරන්න; නවතම සංස්කරණය සැමවිටම මෙහි ඇත.',
        'win_title': 'Windows 10 සහ 11',
        'win_items': [('win-x64', 'බොහෝ පරිගණක සඳහා ස්ථාපකය (x64)'), ('win-arm64', 'ARM මත Windows (ARM64)'),
                      ('win-x64-msi', 'ආයතන සඳහා MSI (x64)'), ('win-arm64-msi', 'MSI (ARM64)')],
        'mac_title': 'macOS 11 හෝ පසු',
        'mac_items': [('mac', 'Apple silicon සහ Intel (.dmg)')],
        'linux_title': 'Linux (x64)',
        'linux_items': [('linux-appimage', 'AppImage (ඕනෑම බෙදාහැරීමක්)'), ('linux-deb', 'Debian, Ubuntu (.deb)'),
                        ('linux-rpm', 'Fedora, openSUSE (.rpm)')],
        'install_guide': 'පියවරෙන් පියවර ස්ථාපන මාර්ගෝපදේශය',
        'all_releases': 'සියලු නිකුතු සහ checksums',
        'faq_title': 'ප්‍රශ්න',
        'faq': [
            ('Lumina නොමිලේද?', 'ඔව්. Lumina GNU General Public License v3.0 යටතේ විවෘත මූලාශ්‍ර වේ. ගෙවිය යුතු සංස්කරණයක් නැත.'),
            ('මට Docker හෝ අතථ්‍ය යන්ත්‍රයක් අවශ්‍යද?', 'නැත. Lumina සැබෑ PHP, Node.js, වෙබ් සේවාදායක සහ දත්ත සමුදා වැඩසටහන් ඔබේ පරිගණකයේම ධාවනය කරයි; එබැවින් එය ඉක්මනින් ආරම්භ වී අඩු මතකයක් භාවිත කරයි.'),
            ('Node.js සහ Python ව්‍යාපෘති සඳහා එය ක්‍රියා කරයිද?', 'ඔව්. Vite, Next.js, Nuxt, Astro, SvelteKit, Django, FastAPI හෝ Flask සහිත ෆෝල්ඩරයක් <code>https://folder.test</code> හි තමන්ගේම dev සේවාදායකයක් මගින් සේවා දෙනු ලැබේ; ව්‍යාපෘතිය <code>.nvmrc</code> හෝ <code>.python-version</code> හි ඉල්ලන Node.js හෝ Python සංස්කරණය සමඟ.'),
            ('මට භාවිත කළ හැක්කේ කුමන PHP සංස්කරණද?', 'Windows මත, ඉක්මන් එකතුවේ ඇති සෑම සංස්කරණයක්ම, එකට. macOS සහ Linux මත, ඔබ Homebrew මගින් ස්ථාපනය කරන සංස්කරණ (උදා: <code>brew install php</code> හෝ <code>php@8.3</code>); Lumina ඒවා තනිවම සොයා ගනී. පරිසරයකට හෝ පැතිකඩකට එක ක්ලික් එකකින් මාරු කරන්න.'),
            ('PostgreSQL සහ MySQL එකට භාවිත කළ හැකිද?', 'ඔව්. PostgreSQL, MySQL හෝ MariaDB සමඟම තමන්ගේම පෝට් එකක ධාවනය වේ.'),
            ('මගේ ව්‍යාපෘති සහ දත්ත සමුදා තබා ඇත්තේ කොහේද?', 'පළමු ආරම්භයේදී ඔබ තෝරන එක් දත්ත ෆෝල්ඩරයක (Windows මත <code>C:\\Lumina</code>, අනෙක් පද්ධතිවල <code>~/Lumina</code>). ඔබ වෙනත් ලෙස නොඉල්ලන්නේ නම් අස්ථාපනය කිරීමේදී එය තබා ගැනේ.'),
            ('එය ආරක්ෂිතද?', 'සේවා සවන් දෙන්නේ ඔබේම පරිගණකයට පමණි. බාගැනීම් තහවුරු කෙරේ, යාවත්කාලීන අත්සන් කර ඇත, පරිපාලක අවසර ඉල්ලන්නේ අවශ්‍ය විට පමණි.'),
        ],
        'footer_tag': 'වෙබ් සංවර්ධකයින් සඳහා දේශීය සංවර්ධන පරිසරයක්.',
        'footer_license': '<a href="https://github.com/SaanPrasanna/Lumina/blob/master/LICENSE">GNU GPL v3.0</a> යටතේ නිකුත් කර ඇත.',
        'footer_links': [('GitHub', REPO), ('නිකුතු', REPO + '/releases'), ('වෙනස්කම් ලේඛනය', BASE + 'changelog/'),
                         ('දෝෂයක් වාර්තා කරන්න', REPO + '/issues/new/choose'), ('ආරක්ෂාව', REPO + '/blob/master/SECURITY.md')],
        'mock_running': 'ධාවනය වේ', 'mock_stopped': 'නවතා ඇත', 'mock_start_all': 'සියල්ල නවත්වන්න',
        'mock_services': 'සේවා',
    },
}


def page(t):
    other = 'si' if t['lang'] == 'en' else 'en'
    up = '../' if t['path'] else ''
    url = BASE + t['path']
    other_url = BASE + T[other]['path']
    og_image = BASE + 'assets/' + t['og']
    # One linked graph: the website and its author, this page (also the FAQ), its image,
    # its breadcrumb and the app itself. Nodes refer to each other by @id.
    ld = {
        '@context': 'https://schema.org',
        '@graph': [
            {
                '@type': 'Person',
                '@id': BASE + '#/schema/person',
                'name': AUTHOR,
                'url': AUTHOR_URL,
                'sameAs': [AUTHOR_URL],
            },
            {
                '@type': 'WebSite',
                '@id': BASE + '#website',
                'url': BASE,
                'name': 'Lumina',
                'description': T['en']['description'],
                'inLanguage': [p['lang'] for p in T.values()],
                'publisher': {'@id': BASE + '#/schema/person'},
            },
            {
                '@type': ['WebPage', 'FAQPage'],
                '@id': url + '#webpage',
                'url': url,
                'name': t['title'],
                'description': t['description'],
                'inLanguage': t['lang'],
                'isPartOf': {'@id': BASE + '#website'},
                'about': {'@id': BASE + '#software'},
                'mainEntity': [{'@type': 'Question', 'name': q,
                                'acceptedAnswer': {'@type': 'Answer', 'text': re.sub('<[^>]+>', '', a)}}
                               for q, a in t['faq']],
                'primaryImageOfPage': {'@id': url + '#primaryimage'},
                'image': {'@id': url + '#primaryimage'},
                'breadcrumb': {'@id': url + '#breadcrumb'},
                'author': {'@id': BASE + '#/schema/person'},
                'datePublished': PUBLISHED,
                'potentialAction': [{'@type': 'ReadAction', 'target': [url]}],
            },
            {
                '@type': 'ImageObject',
                '@id': url + '#primaryimage',
                'url': og_image,
                'contentUrl': og_image,
                'width': 1200,
                'height': 630,
                'caption': t['og_alt'],
                'inLanguage': t['lang'],
            },
            {
                '@type': 'BreadcrumbList',
                '@id': url + '#breadcrumb',
                'itemListElement': [{'@type': 'ListItem', 'position': 1, 'name': 'Lumina', 'item': url}],
            },
            {
                '@type': 'SoftwareApplication',
                '@id': BASE + '#software',
                'name': 'Lumina',
                'alternateName': t['app_kind'],
                'description': t['description'],
                'url': BASE,
                'image': BASE + 'assets/icon-512.png',
                'screenshot': {'@id': url + '#primaryimage'},
                'applicationCategory': 'DeveloperApplication',
                'applicationSubCategory': T['en']['app_kind'],
                'operatingSystem': 'Windows 10, Windows 11, macOS 11+, Linux',
                'processorRequirements': 'x64, ARM64 (Windows), Apple silicon',
                'inLanguage': [p['lang'] for p in T.values()],
                'featureList': [h for _, h, _ in t['features']],
                'offers': {'@type': 'Offer', 'price': '0', 'priceCurrency': 'USD',
                           'availability': 'https://schema.org/InStock', 'url': url + '#download'},
                'isAccessibleForFree': True,
                'license': 'https://www.gnu.org/licenses/gpl-3.0.html',
                'downloadUrl': LATEST,
                'installUrl': url + '#download',
                'softwareVersion': RELEASES[0]['version'],
                'releaseNotes': BASE + 'changelog/',
                'sameAs': [REPO],
                'author': {'@id': BASE + '#/schema/person'},
                'publisher': {'@id': BASE + '#/schema/person'},
            },
        ],
    }
    esc = html.escape
    features = '\n'.join(
        f'''        <li class="card feature">
          <span class="feature-icon">{icon(i)}</span>
          <h3>{esc(h)}</h3>
          <p>{p}</p>
        </li>''' for i, h, p in t['features'])
    stack = '\n'.join(f'          <li>{esc(s)}</li>' for s in STACK)
    steps = '\n'.join(
        f'''        <li class="step">
          <span class="step-n" aria-hidden="true">{n}</span>
          <h3>{esc(h)}</h3>
          <p>{p}</p>
        </li>''' for n, (h, p) in enumerate(t['steps'], 1))

    def dl_card(os_id, ic, title, items):
        links = '\n'.join(
            f'            <li><a class="dl-link" data-asset="{k}" href="{LATEST}">{icon("download", 18)}<span>{esc(label)}</span></a></li>'
            for k, label in items)
        return f'''        <div class="card dl-card" data-os="{os_id}">
          <div class="dl-head">{icon(ic, 26)}<h3>{esc(title)}</h3></div>
          <ul class="dl-list">
{links}
          </ul>
        </div>'''

    faq = '\n'.join(
        f'''        <details class="faq-item">
          <summary>{esc(q)}</summary>
          <p>{a}</p>
        </details>''' for q, a in t['faq'])
    footer_links = ' · '.join(f'<a href="{u}">{esc(n)}</a>' for n, u in t['footer_links'])
    mock_rows = [('PHP', '8.4.26', True), ('nginx', '1.30.5', True), ('MySQL', '8.4.11', True),
                 ('PostgreSQL', '18.6', True), ('Redis', '8.10.2', False)]
    mock = '\n'.join(
        f'''              <div class="m-row"><span class="m-dot{' on' if on else ''}"></span><span class="m-name">{n}</span><span class="m-ver">{v}</span><span class="m-state">{t['mock_running'] if on else t['mock_stopped']}</span></div>'''
        for n, v, on in mock_rows)

    return f'''<!doctype html>
<html lang="{t['lang']}">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{esc(t['title'])}</title>
  <meta name="description" content="{esc(t['description'])}">
  <meta name="robots" content="index, follow, max-image-preview:large, max-snippet:-1, max-video-preview:-1">
  <meta name="author" content="{AUTHOR}">
  <meta name="application-name" content="Lumina">
  <meta name="apple-mobile-web-app-title" content="Lumina">
  <meta name="format-detection" content="telephone=no">
  <link rel="canonical" href="{url}">
  <link rel="alternate" hreflang="en" href="{BASE}">
  <link rel="alternate" hreflang="si" href="{BASE}si/">
  <link rel="alternate" hreflang="x-default" href="{BASE}">
  <meta name="theme-color" content="#1b1e20" media="(prefers-color-scheme: dark)">
  <meta name="theme-color" content="#ffffff" media="(prefers-color-scheme: light)">
  <meta name="color-scheme" content="dark light">
  <link rel="icon" href="{up}assets/icon.svg" type="image/svg+xml">
  <link rel="icon" href="{up}assets/icon-32.png" sizes="32x32" type="image/png">
  <link rel="apple-touch-icon" href="{up}assets/icon-180.png">
  <link rel="manifest" href="{up}site.webmanifest">
  <link rel="author" href="{AUTHOR_URL}">
  <meta property="og:type" content="website">
  <meta property="og:site_name" content="Lumina">
  <meta property="og:title" content="{esc(t['og_title'])}">
  <meta property="og:description" content="{esc(t['description'])}">
  <meta property="og:url" content="{url}">
  <meta property="og:image" content="{og_image}">
  <meta property="og:image:secure_url" content="{og_image}">
  <meta property="og:image:type" content="image/png">
  <meta property="og:image:width" content="1200">
  <meta property="og:image:height" content="630">
  <meta property="og:image:alt" content="{esc(t['og_alt'])}">
  <meta property="og:locale" content="{'en_US' if t['lang'] == 'en' else 'si_LK'}">
  <meta property="og:locale:alternate" content="{'si_LK' if t['lang'] == 'en' else 'en_US'}">
  <meta name="twitter:card" content="summary_large_image">
  <meta name="twitter:title" content="{esc(t['og_title'])}">
  <meta name="twitter:description" content="{esc(t['description'])}">
  <meta name="twitter:image" content="{og_image}">
  <meta name="twitter:image:alt" content="{esc(t['og_alt'])}">
  <link rel="preload" href="{up}assets/fonts/manrope-latin.woff2" as="font" type="font/woff2" crossorigin>
{'  <link rel="preload" href="../assets/fonts/noto-sans-sinhala.woff2" as="font" type="font/woff2" crossorigin>' + chr(10) if t['lang'] == 'si' else ''}  <link rel="stylesheet" href="{up}assets/style.css">
  <script type="application/ld+json">{json.dumps(ld, ensure_ascii=False).replace('</', '<\\/')}</script>
  <script src="{up}assets/site.js" defer></script>
</head>
<body class="lang-{t['lang']}">
  <a class="skip" href="#main">{esc(t['skip'])}</a>
  <header class="top">
    <nav class="wrap nav" aria-label="Main">
      <a class="brand" href="{up or './'}"><img src="{up}assets/icon.svg" alt="" width="30" height="30"><span>Lumina</span></a>
      <div class="nav-links">
        <a href="#features">{esc(t['nav_features'])}</a>
        <a href="#download">{esc(t['nav_download'])}</a>
        <a href="#faq">{esc(t['nav_faq'])}</a>
        <a href="{up}changelog/"{' hreflang="en"' if t['lang'] != 'en' else ''}>{esc(t['nav_changelog'])}</a>
        <a class="lang-switch" href="{up}{T[other]['path'] or './'}" hreflang="{other}" lang="{other}" title="{esc(t['switch_title'])}">{esc(t['switch_label'])}</a>
        <a class="icon-link" href="{REPO}" aria-label="GitHub">{icon('github', 20)}</a>
      </div>
    </nav>
  </header>

  <main id="main">
    <section class="hero wrap">
      <div class="hero-copy">
        <p class="badge">{esc(t['badge'])}</p>
        <h1>{t['h1']}</h1>
        <p class="lead">{t['lead']}</p>
        <div class="cta">
          <a class="btn primary" id="hero-download" href="#download"
             data-label-windows="{esc(t['dl_windows'])}" data-label-macos="{esc(t['dl_macos'])}"
             data-label-linux="{esc(t['dl_linux'])}">{icon('download', 20)}<span>{esc(t['dl_generic'])}</span></a>
          <a class="btn" href="{REPO}">{icon('github', 20)}<span>{esc(t['github_btn'])}</span></a>
        </div>
        <p class="meta-line">{t['meta_line']}</p>
      </div>
      <div class="hero-art" aria-hidden="true">
        <div class="window">
          <div class="w-bar"><span></span><span></span><span></span><b>Lumina</b></div>
          <div class="w-body">
            <div class="w-side"><i class="act"></i><i></i><i></i><i></i><i></i></div>
            <div class="w-main">
              <div class="w-head"><strong>{esc(t['mock_services'])}</strong><span class="w-btn">{esc(t['mock_start_all'])}</span></div>
{mock}
              <div class="w-url">https://blog.test <span>🔒</span></div>
            </div>
          </div>
        </div>
      </div>
    </section>

    <section class="stack wrap" aria-labelledby="stack-title">
      <h2 id="stack-title" class="eyebrow">{esc(t['stack_title'])}</h2>
      <ul class="chips">
{stack}
      </ul>
    </section>

    <section id="features" class="section wrap" aria-labelledby="features-title">
      <h2 id="features-title">{esc(t['features_title'])}</h2>
      <p class="section-lead">{esc(t['features_lead'])}</p>
      <ul class="grid features">
{features}
      </ul>
    </section>

    <section class="section wrap" aria-labelledby="how-title">
      <h2 id="how-title">{esc(t['how_title'])}</h2>
      <ol class="grid steps">
{steps}
      </ol>
    </section>

    <section id="download" class="section wrap" aria-labelledby="download-title">
      <h2 id="download-title">{esc(t['download_title'])} <span class="version" data-version></span></h2>
      <p class="section-lead">{esc(t['download_lead'])}</p>
      <div class="grid downloads">
{dl_card('windows', 'windows', t['win_title'], t['win_items'])}
{dl_card('macos', 'apple', t['mac_title'], t['mac_items'])}
{dl_card('linux', 'linux', t['linux_title'], t['linux_items'])}
      </div>
      <p class="dl-more"><a href="{REPO}/blob/master/INSTALL.md">{esc(t['install_guide'])}</a> · <a href="{REPO}/releases">{esc(t['all_releases'])}</a></p>
    </section>

    <section id="faq" class="section wrap narrow" aria-labelledby="faq-title">
      <h2 id="faq-title">{esc(t['faq_title'])}</h2>
{faq}
    </section>
  </main>

  <footer class="foot">
    <div class="wrap foot-inner">
      <div>
        <a class="brand" href="{up or './'}"><img src="{up}assets/icon.svg" alt="" width="26" height="26"><span>Lumina</span></a>
        <p>{esc(t['footer_tag'])}</p>
      </div>
      <div class="foot-links">
        <p>{footer_links}</p>
        <p>© 2026 Sandun Prasanna. {t['footer_license']}</p>
      </div>
    </div>
  </footer>
</body>
</html>
'''


def not_found():
    """404.html: GitHub Pages serves it for every missing address at any depth,
    so its links are absolute. Both languages on one page."""
    site = '/'
    return f'''<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Page not found · Lumina</title>
  <meta name="robots" content="noindex">
  <meta name="color-scheme" content="dark light">
  <link rel="icon" href="{site}assets/icon.svg" type="image/svg+xml">
  <link rel="icon" href="{site}assets/icon-32.png" sizes="32x32" type="image/png">
  <link rel="preload" href="{site}assets/fonts/manrope-latin.woff2" as="font" type="font/woff2" crossorigin>
  <link rel="stylesheet" href="{site}assets/style.css">
</head>
<body>
  <header class="top">
    <nav class="wrap nav" aria-label="Main">
      <a class="brand" href="{site}"><img src="{site}assets/icon.svg" alt="" width="30" height="30"><span>Lumina</span></a>
      <div class="nav-links">
        <a href="{site}#download">Download</a>
        <a href="{site}changelog/">Changelog</a>
        <a class="icon-link" href="{REPO}" aria-label="GitHub">{icon('github', 20)}</a>
      </div>
    </nav>
  </header>

  <main id="main" class="wrap narrow not-found">
    <p class="nf-code" aria-hidden="true">404</p>
    <h1>Page not found</h1>
    <p class="lead">The page you were looking for does not exist or has moved.</p>
    <h2 lang="si">පිටුව හමු නොවීය</h2>
    <p class="lead" lang="si">ඔබ සෙවූ පිටුව නොපවතී, නැතහොත් වෙනත් තැනකට ගෙන ගොස් ඇත.</p>
    <div class="cta">
      <a class="btn primary" href="{site}"><span>Go to the home page</span></a>
      <a class="btn" href="{site}si/" lang="si"><span>මුල් පිටුවට යන්න</span></a>
    </div>
  </main>

  <footer class="foot">
    <div class="wrap foot-inner">
      <div>
        <a class="brand" href="{site}"><img src="{site}assets/icon.svg" alt="" width="26" height="26"><span>Lumina</span></a>
        <p>A local development environment for web developers.</p>
      </div>
      <div class="foot-links">
        <p><a href="{REPO}">GitHub</a> · <a href="{REPO}/releases">Releases</a> · <a href="{site}changelog/">Changelog</a></p>
        <p>© 2026 Sandun Prasanna. Released under the <a href="{REPO}/blob/master/LICENSE">GNU GPL v3.0</a>.</p>
      </div>
    </div>
  </footer>
</body>
</html>
'''


CHANGELOG = os.path.join(ROOT, '..', 'CHANGELOG.md')
MONTHS = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August',
          'September', 'October', 'November', 'December']


def md_inline(text):
    """The Markdown CHANGELOG.md uses inside a line: `code`, **bold** and [links](url)."""
    out = html.escape(text, quote=False)
    out = re.sub(r'`([^`]+)`', r'<code>\1</code>', out)
    out = re.sub(r'\*\*(.+?)\*\*', r'<strong>\1</strong>', out)
    return re.sub(r'\[([^\]]+)\]\(([^)\s]+)\)', r'<a href="\2">\1</a>', out)


def parse_changelog():
    """[{version, date, intro, items}] from CHANGELOG.md, newest first. "Unreleased" is left out."""
    releases = []
    for block in re.split(r'^## ', open(CHANGELOG, encoding='utf8').read(), flags=re.M)[1:]:
        head, _, body = block.partition('\n')
        m = re.match(r'\[(\d+\.\d+\.\d+)\] - (\d{4}-\d{2}-\d{2})', head)
        if not m:
            continue
        intro, items = [], []
        for line in body.strip().splitlines():
            if line.startswith('- '):
                items.append(line[2:].strip())
            elif line.strip() and not items:
                intro.append(line.strip())
            elif line.strip():
                items[-1] += ' ' + line.strip()
        releases.append({'version': m[1], 'date': m[2], 'intro': ' '.join(intro), 'items': items})
    return releases


def long_date(d):
    y, mo, day = d.split('-')
    return f'{MONTHS[int(mo) - 1]} {int(day)}, {y}'


def release_html(rel):
    items = '\n'.join(f'            <li>{md_inline(i)}</li>' for i in rel['items'])
    intro = f'\n          <p class="rel-intro">{md_inline(rel["intro"])}</p>' if rel['intro'] else ''
    return intro, items


def changelog_page():
    """site/changelog/index.html: every release from CHANGELOG.md, in English."""
    url = BASE + 'changelog/'
    title = 'Changelog · Lumina'
    description = ('Every Lumina release and what changed in it: new features, improvements and fixes, '
                   'newest first.')
    og_image = BASE + 'assets/og-en.png'
    esc = html.escape
    latest = RELEASES[0]
    ld = {
        '@context': 'https://schema.org',
        '@graph': [
            {
                '@type': 'WebPage',
                '@id': url + '#webpage',
                'url': url,
                'name': title,
                'description': description,
                'inLanguage': 'en',
                'isPartOf': {'@id': BASE + '#website'},
                'about': {'@id': BASE + '#software'},
                'breadcrumb': {'@id': url + '#breadcrumb'},
                'dateModified': latest['date'],
            },
            {
                '@type': 'BreadcrumbList',
                '@id': url + '#breadcrumb',
                'itemListElement': [
                    {'@type': 'ListItem', 'position': 1, 'name': 'Lumina', 'item': BASE},
                    {'@type': 'ListItem', 'position': 2, 'name': 'Changelog', 'item': url},
                ],
            },
        ],
    }
    toc = '\n'.join(
        f'          <li><a href="#v{r["version"]}"><span>{r["version"]}</span><time datetime="{r["date"]}">{long_date(r["date"])}</time></a></li>'
        for r in RELEASES)
    articles = []
    for n, rel in enumerate(RELEASES):
        intro, items = release_html(rel)
        v = rel['version']
        badge = '<span class="rel-badge">Latest</span>' if n == 0 else ''
        articles.append(f'''        <article id="v{v}" class="card release" aria-labelledby="v{v}-title">
          <header class="rel-head">
            <h2 id="v{v}-title"><a href="#v{v}">Lumina {v}</a></h2>{badge}
            <time datetime="{rel['date']}">{long_date(rel['date'])}</time>
          </header>{intro}
          <ul class="rel-items">
{items}
          </ul>
          <p class="rel-links"><a href="{REPO}/releases/tag/v{v}">Downloads and checksums for {v}</a></p>
        </article>''')
    articles = '\n'.join(articles)
    return f'''<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{title}</title>
  <meta name="description" content="{esc(description)}">
  <meta name="robots" content="index, follow, max-image-preview:large, max-snippet:-1">
  <meta name="author" content="{AUTHOR}">
  <meta name="application-name" content="Lumina">
  <link rel="canonical" href="{url}">
  <link rel="alternate" type="application/atom+xml" title="Lumina releases" href="{url}feed.xml">
  <meta name="theme-color" content="#1b1e20" media="(prefers-color-scheme: dark)">
  <meta name="theme-color" content="#ffffff" media="(prefers-color-scheme: light)">
  <meta name="color-scheme" content="dark light">
  <link rel="icon" href="../assets/icon.svg" type="image/svg+xml">
  <link rel="icon" href="../assets/icon-32.png" sizes="32x32" type="image/png">
  <link rel="apple-touch-icon" href="../assets/icon-180.png">
  <link rel="manifest" href="../site.webmanifest">
  <meta property="og:type" content="website">
  <meta property="og:site_name" content="Lumina">
  <meta property="og:title" content="{title}">
  <meta property="og:description" content="{esc(description)}">
  <meta property="og:url" content="{url}">
  <meta property="og:image" content="{og_image}">
  <meta property="og:image:width" content="1200">
  <meta property="og:image:height" content="630">
  <meta property="og:locale" content="en_US">
  <meta name="twitter:card" content="summary_large_image">
  <meta name="twitter:title" content="{title}">
  <meta name="twitter:description" content="{esc(description)}">
  <meta name="twitter:image" content="{og_image}">
  <link rel="preload" href="../assets/fonts/manrope-latin.woff2" as="font" type="font/woff2" crossorigin>
  <link rel="stylesheet" href="../assets/style.css">
  <script type="application/ld+json">{json.dumps(ld, ensure_ascii=False).replace('</', '<\\/')}</script>
</head>
<body class="lang-en">
  <a class="skip" href="#main">Skip to content</a>
  <header class="top">
    <nav class="wrap nav" aria-label="Main">
      <a class="brand" href="../"><img src="../assets/icon.svg" alt="" width="30" height="30"><span>Lumina</span></a>
      <div class="nav-links">
        <a href="../#features">Features</a>
        <a href="../#download">Download</a>
        <a href="../#faq">FAQ</a>
        <a href="./" aria-current="page">Changelog</a>
        <a class="icon-link" href="{REPO}" aria-label="GitHub">{icon('github', 20)}</a>
      </div>
    </nav>
  </header>

  <main id="main" class="wrap changelog">
    <nav class="crumbs" aria-label="Breadcrumb"><a href="../">Lumina</a><span aria-hidden="true">/</span><span aria-current="page">Changelog</span></nav>
    <header class="cl-head">
      <h1>Changelog</h1>
      <p class="lead">Every Lumina release and what changed in it, newest first. Lumina updates itself; every version can also be downloaded from GitHub.</p>
      <div class="cta">
        <a class="btn primary" href="../#download">{icon('download', 20)}<span>Download Lumina {latest['version']}</span></a>
        <a class="btn" href="feed.xml">{icon('rss', 20)}<span>Subscribe (Atom feed)</span></a>
      </div>
    </header>
    <div class="cl-layout">
      <nav class="cl-toc" aria-label="Releases">
        <p class="eyebrow">Releases</p>
        <ul>
{toc}
        </ul>
      </nav>
      <div class="cl-list">
{articles}
      </div>
    </div>
  </main>

  <footer class="foot">
    <div class="wrap foot-inner">
      <div>
        <a class="brand" href="../"><img src="../assets/icon.svg" alt="" width="26" height="26"><span>Lumina</span></a>
        <p>{esc(T['en']['footer_tag'])}</p>
      </div>
      <div class="foot-links">
        <p>{' · '.join(f'<a href="{u}">{esc(n)}</a>' for n, u in T['en']['footer_links'])}</p>
        <p>© 2026 Sandun Prasanna. {T['en']['footer_license']}</p>
      </div>
    </div>
  </footer>
</body>
</html>
'''


def atom_feed():
    """site/changelog/feed.xml: one entry per release, for feed readers."""
    url = BASE + 'changelog/'
    esc = html.escape
    entries = []
    for rel in RELEASES:
        intro, items = release_html(rel)
        content = f'{intro}\n<ul>\n{items}\n</ul>'
        entries.append(f'''  <entry>
    <title>Lumina {rel['version']}</title>
    <link rel="alternate" type="text/html" href="{url}#v{rel['version']}"/>
    <id>tag:lumina.hexdev.tech,{rel['date']}:v{rel['version']}</id>
    <published>{rel['date']}T00:00:00Z</published>
    <updated>{rel['date']}T00:00:00Z</updated>
    <summary>{esc(re.sub('<[^>]+>', '', md_inline(rel['intro'])) or 'Lumina ' + rel['version'])}</summary>
    <content type="html">{esc(content)}</content>
  </entry>''')
    entries = '\n'.join(entries)
    return f'''<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xml:lang="en">
  <title>Lumina releases</title>
  <subtitle>New versions of Lumina and what changed in them.</subtitle>
  <link rel="self" type="application/atom+xml" href="{url}feed.xml"/>
  <link rel="alternate" type="text/html" href="{url}"/>
  <id>{url}</id>
  <updated>{RELEASES[0]['date']}T00:00:00Z</updated>
  <author><name>{AUTHOR}</name><uri>{AUTHOR_URL}</uri></author>
  <icon>{BASE}assets/icon-180.png</icon>
  <logo>{BASE}assets/icon-512.png</logo>
  <rights>GNU GPL v3.0</rights>
{entries}
</feed>
'''


def git_date(path):
    """Last commit time of a file as a UTC datetime, or None."""
    try:
        out = subprocess.run(['git', 'log', '-1', '--format=%cI', '--', path], cwd=ROOT,
                             capture_output=True, text=True, check=True).stdout.strip()
        return datetime.fromisoformat(out).astimezone(timezone.utc) if out else None
    except (OSError, subprocess.CalledProcessError, ValueError):
        return None


def iso(d):
    return d.strftime('%Y-%m-%dT%H:%M:%S+00:00')


def write(path, text):
    """Writes the file and returns True when its contents changed."""
    try:
        changed = open(path, encoding='utf8').read() != text
    except FileNotFoundError:
        changed = True
    if changed:
        open(path, 'w', encoding='utf8', newline='\n').write(text)
    print('wrote' if changed else 'unchanged', path)
    return changed


def old_lastmods():
    """loc -> lastmod from the page sitemap already on disk."""
    try:
        xml = open(ROOT + 'page-sitemap.xml', encoding='utf8').read()
    except FileNotFoundError:
        return {}
    return dict(re.findall(r'<loc>([^<]+)</loc>\s*<lastmod>([^<]+)</lastmod>', xml))


def page_sitemap(lastmods):
    """Every page with its last change, its language versions and its images."""
    alternates = ''.join(
        f'\n    <xhtml:link rel="alternate" hreflang="{lang}" href="{BASE + p["path"]}"/>'
        for lang, p in T.items()) + f'\n    <xhtml:link rel="alternate" hreflang="x-default" href="{BASE}"/>'
    urls = '\n'.join(f'''  <url>
    <loc>{BASE + t['path']}</loc>
    <lastmod>{lastmods[BASE + t['path']]}</lastmod>{alternates}
    <image:image>
      <image:loc>{BASE}assets/{t['og']}</image:loc>
    </image:image>
    <image:image>
      <image:loc>{BASE}assets/icon-512.png</image:loc>
    </image:image>
  </url>''' for t in T.values())
    # The changelog is in English only: no language versions.
    urls += f'''
  <url>
    <loc>{BASE}changelog/</loc>
    <lastmod>{lastmods[BASE + 'changelog/']}</lastmod>
    <image:image>
      <image:loc>{BASE}assets/og-en.png</image:loc>
    </image:image>
  </url>'''
    return f'''<?xml version="1.0" encoding="UTF-8"?>
<?xml-stylesheet type="text/xsl" href="/sitemap.xsl"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"
        xmlns:xhtml="http://www.w3.org/1999/xhtml"
        xmlns:image="http://www.google.com/schemas/sitemap-image/1.1">
{urls}
</urlset>
'''


def sitemap_index(lastmod):
    return f'''<?xml version="1.0" encoding="UTF-8"?>
<?xml-stylesheet type="text/xsl" href="/sitemap.xsl"?>
<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <sitemap>
    <loc>{BASE}page-sitemap.xml</loc>
    <lastmod>{lastmod}</lastmod>
  </sitemap>
</sitemapindex>
'''


SITEMAP_XSL = '''<?xml version="1.0" encoding="UTF-8"?>
<!-- Shows the sitemaps as a readable page in a browser. Search engines read the XML itself. -->
<xsl:stylesheet version="1.0"
    xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
    xmlns:s="http://www.sitemaps.org/schemas/sitemap/0.9"
    xmlns:xhtml="http://www.w3.org/1999/xhtml"
    xmlns:image="http://www.google.com/schemas/sitemap-image/1.1"
    exclude-result-prefixes="s xhtml image">
  <xsl:output method="html" encoding="UTF-8" indent="yes" doctype-system="about:legacy-compat"/>

  <xsl:template name="date">
    <xsl:param name="d"/>
    <xsl:value-of select="concat(substring($d, 1, 10), ' ', substring($d, 12, 5), ' UTC')"/>
  </xsl:template>

  <xsl:template match="/">
    <html lang="en">
      <head>
        <meta charset="utf-8"/>
        <meta name="viewport" content="width=device-width, initial-scale=1"/>
        <meta name="robots" content="noindex, follow"/>
        <meta name="color-scheme" content="dark light"/>
        <title>XML Sitemap · Lumina</title>
        <link rel="icon" href="/assets/icon.svg" type="image/svg+xml"/>
        <style>
          @font-face { font-family: "Manrope"; src: url("/assets/fonts/manrope-latin.woff2") format("woff2"); font-weight: 200 800; font-display: swap; }
          :root { --surface: #ffffff; --raised: #f7f7f5; --fg: #1d1d1f; --muted: #6a6a6f; --line: #e4e4e1;
                  --accent-soft: #fdf3e3; --accent-ink: #a8530a; color-scheme: light; }
          @media (prefers-color-scheme: dark) {
            :root { --surface: #1b1e20; --raised: #202325; --fg: #eae7e2; --muted: #959a9f; --line: #2b2f33;
                    --accent-soft: #33291a; --accent-ink: #d8a44c; color-scheme: dark; }
          }
          * { box-sizing: border-box; }
          body { margin: 0; background: var(--surface); color: var(--fg);
                 font: 15px/1.6 "Manrope", system-ui, "Segoe UI", -apple-system, sans-serif; }
          .wrap { max-width: 1080px; margin: 0 auto; padding: 40px 16px 64px; }
          .brand { display: inline-flex; align-items: center; gap: 10px; color: inherit; text-decoration: none; font-weight: 700; font-size: 18px; }
          h1 { margin: 28px 0 8px; font-size: clamp(26px, 4vw, 34px); line-height: 1.2; letter-spacing: -0.01em; }
          .intro { margin: 0 0 6px; color: var(--muted); max-width: 70ch; }
          a { color: var(--accent-ink); }
          .count { display: inline-block; margin: 18px 0 14px; padding: 4px 12px; border-radius: 999px;
                   background: var(--accent-soft); color: var(--accent-ink); font-weight: 600; font-size: 13px; }
          .table { border: 1px solid var(--line); border-radius: 11px; overflow-x: auto; background: var(--raised); }
          table { width: 100%; border-collapse: collapse; }
          th, td { padding: 12px 16px; text-align: left; vertical-align: top; }
          th { font-size: 12px; text-transform: uppercase; letter-spacing: 0.06em; color: var(--muted); border-bottom: 1px solid var(--line); white-space: nowrap; }
          tr + tr td { border-top: 1px solid var(--line); }
          td a { text-decoration: none; word-break: break-all; font-weight: 600; }
          td a:hover { text-decoration: underline; }
          .num, .date { white-space: nowrap; color: var(--muted); font-variant-numeric: tabular-nums; }
          .langs { display: flex; flex-wrap: wrap; gap: 6px; }
          .langs a { padding: 1px 8px; border: 1px solid var(--line); border-radius: 6px; font-size: 12px; word-break: normal; }
          .foot { margin-top: 24px; color: var(--muted); font-size: 13px; }
          @media (max-width: 640px) { .hide-sm { display: none; } th, td { padding: 10px 12px; } }
        </style>
      </head>
      <body>
        <main class="wrap">
          <a class="brand" href="/"><img src="/assets/icon.svg" alt="" width="28" height="28"/><span>Lumina</span></a>
          <h1>XML Sitemap</h1>
          <p class="intro">This sitemap lists the pages of this website for search engines such as Google and Bing,
            so they can find every page and see when it last changed.</p>
          <xsl:choose>
            <xsl:when test="s:sitemapindex">
              <span class="count"><xsl:value-of select="count(s:sitemapindex/s:sitemap)"/> sitemap(s)</span>
              <div class="table">
                <table>
                  <thead><tr><th>Sitemap</th><th>Last modified</th></tr></thead>
                  <tbody>
                    <xsl:for-each select="s:sitemapindex/s:sitemap">
                      <tr>
                        <td><a href="{s:loc}"><xsl:value-of select="s:loc"/></a></td>
                        <td class="date"><xsl:call-template name="date"><xsl:with-param name="d" select="s:lastmod"/></xsl:call-template></td>
                      </tr>
                    </xsl:for-each>
                  </tbody>
                </table>
              </div>
            </xsl:when>
            <xsl:otherwise>
              <p class="intro"><a href="/sitemap.xml">← Sitemap index</a></p>
              <span class="count"><xsl:value-of select="count(s:urlset/s:url)"/> URL(s)</span>
              <div class="table">
                <table>
                  <thead><tr><th>URL</th><th class="hide-sm">Languages</th><th class="hide-sm">Images</th><th>Last modified</th></tr></thead>
                  <tbody>
                    <xsl:for-each select="s:urlset/s:url">
                      <tr>
                        <td><a href="{s:loc}"><xsl:value-of select="s:loc"/></a></td>
                        <td class="hide-sm">
                          <div class="langs">
                            <xsl:for-each select="xhtml:link[@rel='alternate']">
                              <a href="{@href}" title="{@href}"><xsl:value-of select="@hreflang"/></a>
                            </xsl:for-each>
                          </div>
                        </td>
                        <td class="num hide-sm"><xsl:value-of select="count(image:image)"/></td>
                        <td class="date"><xsl:call-template name="date"><xsl:with-param name="d" select="s:lastmod"/></xsl:call-template></td>
                      </tr>
                    </xsl:for-each>
                  </tbody>
                </table>
              </div>
            </xsl:otherwise>
          </xsl:choose>
          <p class="foot">Learn more about sitemaps at <a href="https://www.sitemaps.org/">sitemaps.org</a>.</p>
        </main>
      </body>
    </html>
  </xsl:template>
</xsl:stylesheet>
'''


RELEASES = parse_changelog()
os.makedirs(ROOT + 'si', exist_ok=True)
os.makedirs(ROOT + 'changelog', exist_ok=True)
now = iso(datetime.now(timezone.utc))
first_run = not os.path.exists(ROOT + 'page-sitemap.xml')
previous = old_lastmods()
lastmods = {}
pages = [(ROOT + t['path'] + 'index.html', BASE + t['path'], page(t)) for t in T.values()]
pages.append((ROOT + 'changelog/index.html', BASE + 'changelog/', changelog_page()))
for out, loc, text in pages:
    changed = write(out, text)
    if loc in previous and not changed:
        lastmods[loc] = previous[loc]
    else:
        # A change is dated now; on the first run the page's last commit is used.
        known = git_date(out) if first_run and not changed else None
        lastmods[loc] = iso(known) if known else now

write(ROOT + 'changelog/feed.xml', atom_feed())
write(ROOT + '404.html', not_found())
write(ROOT + 'page-sitemap.xml', page_sitemap(lastmods))
write(ROOT + 'sitemap.xml', sitemap_index(max(lastmods.values())))
write(ROOT + 'sitemap.xsl', SITEMAP_XSL)
write(ROOT + 'site.webmanifest', json.dumps({
    'name': 'Lumina',
    'short_name': 'Lumina',
    'description': T['en']['description'],
    'start_url': '/',
    'scope': '/',
    'display': 'browser',
    'lang': 'en',
    'background_color': '#1b1e20',
    'theme_color': '#1b1e20',
    'icons': [
        {'src': '/assets/icon-180.png', 'sizes': '180x180', 'type': 'image/png'},
        {'src': '/assets/icon-512.png', 'sizes': '512x512', 'type': 'image/png'},
        {'src': '/assets/icon.svg', 'sizes': 'any', 'type': 'image/svg+xml'},
    ],
}, indent=2) + '\n')
