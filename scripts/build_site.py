"""Generates the website pages from the texts below, so both languages keep
the same layout: site/index.html (English), site/si/index.html (Sinhala) and
site/404.html.

    python scripts/build_site.py

Edit the texts or the templates here, run the script and commit the result.
The styles are in site/assets/style.css, the download links in site/assets/site.js.
"""
import html
import json
import os

ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'site')) + os.sep
BASE = 'https://lumina.hexdev.tech/'
REPO = 'https://github.com/SaanPrasanna/Lumina'
LATEST = REPO + '/releases/latest'

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
}


def icon(name, size=22):
    return (f'<svg class="i" width="{size}" height="{size}" viewBox="0 0 24 24" fill="none" '
            f'stroke="currentColor" stroke-width="1.8" stroke-linecap="round" '
            f'stroke-linejoin="round" aria-hidden="true">{ICONS[name]}</svg>')


STACK = ['PHP', 'nginx', 'Apache', 'MySQL', 'MariaDB', 'PostgreSQL', 'Redis',
         'Node.js', 'Python', 'Composer', 'phpMyAdmin', 'ngrok']

T = {
    'en': {
        'lang': 'en', 'dir': '', 'path': '', 'og': 'og-en.png',
        'title': 'Lumina – free local PHP development environment for Windows, macOS and Linux',
        'description': ('Lumina is a free, open-source local development environment for PHP. '
                        'Run nginx or Apache, MySQL, MariaDB, PostgreSQL and Redis, with automatic '
                        '.test sites and trusted HTTPS, on Windows, macOS and Linux.'),
        'og_title': 'Lumina – local PHP development, one click away',
        'nav_features': 'Features', 'nav_download': 'Download', 'nav_faq': 'FAQ',
        'switch_label': 'සිංහල', 'switch_title': 'සිංහලෙන් කියවන්න',
        'skip': 'Skip to content',
        'badge': 'Free and open source',
        'h1': 'Your PHP stack,<br><span class="hl">one click away</span>',
        'lead': ('Lumina runs PHP, a web server and your databases on your own computer. '
                 'Every project gets its own <code>https://name.test</code> address. '
                 'No Docker, no virtual machine, nothing left running when you quit.'),
        'dl_generic': 'Download Lumina', 'dl_windows': 'Download for Windows',
        'dl_macos': 'Download for macOS', 'dl_linux': 'Download for Linux',
        'github_btn': 'View on GitHub',
        'meta_line': 'Windows · macOS · Linux · <span data-version>latest</span>',
        'stack_title': 'Everything a PHP project needs',
        'features_title': 'Built for everyday development',
        'features_lead': 'Fast to start, quiet in the background, and out of your way.',
        'features': [
            ('stack', 'One-click stack', 'PHP with nginx or Apache, MySQL or MariaDB, PostgreSQL and Redis. Start All and Stop All from the app or the tray.'),
            ('globe', 'Automatic sites', 'Every folder in your projects folder becomes <code>folder.test</code>. Lumina keeps the hosts file up to date.'),
            ('lock', 'Trusted local HTTPS', 'A certificate authority that exists only on your computer gives every site a padlock your browsers trust.'),
            ('package', 'Quick Add', 'Install and switch versions of PHP, servers, databases, Node.js, Python and Composer. Every download is checked against its SHA-256. On macOS and Linux, PHP and the web servers come from Homebrew.'),
            ('rocket', 'QuickApp', 'Laravel, WordPress, Symfony, Drupal, Joomla and more in one click, with the database created and the site ready.'),
            ('layers', 'Profiles', 'Save stacks like “nginx · PHP 8.3 · MySQL 8.4” and switch between them in one click.'),
            ('menu', 'Tray control panel', 'A menu for each service: start or stop it, pick its version, open its config file or its log.'),
            ('share', 'Share a site', 'Put a site on a public HTTPS address through ngrok, with a QR code for your phone.'),
            ('refresh', 'Signed updates', 'Lumina updates itself. Every update is signed and verified first, and your services start again afterwards.'),
        ],
        'how_title': 'Up and running in minutes',
        'steps': [
            ('Install Lumina', 'Download the installer for your system and run it. No administrator rights needed on Windows.'),
            ('Add your tools', 'Open Quick Add and install PHP, a web server and a database, in the versions you need.'),
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
            ('Do I need Docker or a virtual machine?', 'No. Lumina runs the real PHP, web server and database programs directly on your computer, so it starts fast and uses little memory.'),
            ('Which PHP versions can I use?', 'On Windows, every version in Quick Add, side by side. On macOS and Linux, the versions you install with Homebrew (for example <code>brew install php</code> or <code>php@8.3</code>), which Lumina finds on its own. Switch with one click, per stack or per profile.'),
            ('Can I use PostgreSQL and MySQL together?', 'Yes. PostgreSQL runs next to MySQL or MariaDB, each on its own port.'),
            ('Where are my projects and databases kept?', 'In one data folder you choose on first start (<code>C:\\Lumina</code> on Windows, <code>~/Lumina</code> elsewhere). Uninstalling keeps it unless you ask otherwise.'),
            ('Is it safe?', 'Services listen only on your own computer. Downloads are verified, updates are signed, and administrator rights are asked for only when they are needed.'),
        ],
        'footer_tag': 'A local development environment for PHP.',
        'footer_license': 'Released under the <a href="https://github.com/SaanPrasanna/Lumina/blob/master/LICENSE">GNU GPL v3.0</a>.',
        'footer_links': [('GitHub', REPO), ('Releases', REPO + '/releases'), ('Changelog', REPO + '/blob/master/CHANGELOG.md'),
                         ('Report a bug', REPO + '/issues/new/choose'), ('Security', REPO + '/blob/master/SECURITY.md')],
        'mock_running': 'Running', 'mock_stopped': 'Stopped', 'mock_start_all': 'Stop All',
        'mock_services': 'Services',
    },
    'si': {
        'lang': 'si', 'dir': '', 'path': 'si/', 'og': 'og-si.png',
        'title': 'Lumina – Windows, macOS සහ Linux සඳහා නොමිලේ PHP සංවර්ධන පරිසරය',
        'description': ('Lumina යනු PHP සඳහා නොමිලේ ලබා ගත හැකි විවෘත මූලාශ්‍ර දේශීය සංවර්ධන පරිසරයකි. '
                        'nginx හෝ Apache, MySQL, MariaDB, PostgreSQL සහ Redis ධාවනය කරන්න; ස්වයංක්‍රීය '
                        '.test අඩවි සහ විශ්වාසනීය HTTPS සමඟ, Windows, macOS සහ Linux මත.'),
        'og_title': 'Lumina – ඔබේ PHP පරිසරය, එක ක්ලික් එකකින්',
        'nav_features': 'විශේෂාංග', 'nav_download': 'බාගන්න', 'nav_faq': 'ප්‍රශ්න',
        'switch_label': 'English', 'switch_title': 'Read in English',
        'skip': 'අන්තර්ගතයට යන්න',
        'badge': 'නොමිලේ සහ විවෘත මූලාශ්‍ර',
        'h1': 'ඔබේ PHP පරිසරය,<br><span class="hl">එක ක්ලික් එකකින්</span>',
        'lead': ('Lumina ඔබේම පරිගණකයේ PHP, වෙබ් සේවාදායකයක් සහ ඔබේ දත්ත සමුදා ධාවනය කරයි. '
                 'සෑම ව්‍යාපෘතියකටම තමන්ගේම <code>https://name.test</code> ලිපිනයක් ලැබේ. '
                 'Docker නැත, අතථ්‍ය යන්ත්‍ර නැත; ඔබ ඉවත් වූ විට කිසිවක් ධාවනය වෙමින් නොපවතී.'),
        'dl_generic': 'Lumina බාගන්න', 'dl_windows': 'Windows සඳහා බාගන්න',
        'dl_macos': 'macOS සඳහා බාගන්න', 'dl_linux': 'Linux සඳහා බාගන්න',
        'github_btn': 'GitHub හි බලන්න',
        'meta_line': 'Windows · macOS · Linux · <span data-version>නවතම</span>',
        'stack_title': 'PHP ව්‍යාපෘතියකට අවශ්‍ය සියල්ල',
        'features_title': 'දිනපතා සංවර්ධනය සඳහා',
        'features_lead': 'ඉක්මනින් ආරම්භ වේ, පසුබිමේ නිශ්ශබ්දව ක්‍රියා කරයි, ඔබට බාධා නොකරයි.',
        'features': [
            ('stack', 'එක් ක්ලික් පරිසරය', 'nginx හෝ Apache, MySQL හෝ MariaDB, PostgreSQL සහ Redis සමඟ PHP. යෙදුමෙන් හෝ පද්ධති තැටියෙන් සියල්ල ආරම්භ කරන්න, නවත්වන්න.'),
            ('globe', 'ස්වයංක්‍රීය අඩවි', 'ඔබේ ව්‍යාපෘති ෆෝල්ඩරයේ සෑම ෆෝල්ඩරයක්ම <code>folder.test</code> බවට පත් වේ. Lumina hosts ගොනුව යාවත්කාලීනව තබයි.'),
            ('lock', 'විශ්වාසනීය දේශීය HTTPS', 'ඔබේ පරිගණකයේ පමණක් පවතින සහතික අධිකාරියක් සෑම අඩවියකටම බ්‍රව්සර විශ්වාස කරන ආරක්ෂිත අගුලක් ලබා දෙයි.'),
            ('package', 'ඉක්මන් එකතුව', 'PHP, සේවාදායක, දත්ත සමුදා, Node.js, Python සහ Composer සංස්කරණ ස්ථාපනය කර මාරු කරන්න. සෑම බාගැනීමක්ම SHA-256 මගින් පරීක්ෂා කෙරේ. macOS සහ Linux මත PHP සහ වෙබ් සේවාදායක Homebrew මගින් ලැබේ.'),
            ('rocket', 'QuickApp', 'Laravel, WordPress, Symfony, Drupal, Joomla සහ තවත් බොහෝ දේ එක ක්ලික් එකකින්; දත්ත සමුදාය සාදා, අඩවිය සූදානම්.'),
            ('layers', 'පැතිකඩ', '“nginx · PHP 8.3 · MySQL 8.4” වැනි පරිසර සුරකින්න, එක ක්ලික් එකකින් ඒවා අතර මාරු වන්න.'),
            ('menu', 'පද්ධති තැටි පාලක පුවරුව', 'සෑම සේවාවකටම මෙනුවක්: ආරම්භ කරන්න හෝ නවත්වන්න, සංස්කරණය තෝරන්න, වින්‍යාස ගොනුව හෝ ලොගය විවෘත කරන්න.'),
            ('share', 'අඩවියක් බෙදාගන්න', 'ngrok හරහා අඩවියක් පොදු HTTPS ලිපිනයකට දමන්න; ඔබේ දුරකථනය සඳහා QR කේතයක් ද සමඟ.'),
            ('refresh', 'අත්සන් කළ යාවත්කාලීන', 'Lumina තමන්ම යාවත්කාලීන වේ. සෑම යාවත්කාලීනයක්ම අත්සන් කර මුලින්ම තහවුරු කෙරේ; පසුව ඔබේ සේවා නැවත ආරම්භ වේ.'),
        ],
        'how_title': 'මිනිත්තු කිහිපයකින් ක්‍රියාත්මකයි',
        'steps': [
            ('Lumina ස්ථාපනය කරන්න', 'ඔබේ පද්ධතියට ගැළපෙන ස්ථාපකය බාගෙන ධාවනය කරන්න. Windows මත පරිපාලක අවසර අවශ්‍ය නැත.'),
            ('මෙවලම් එකතු කරන්න', 'ඉක්මන් එකතුව විවෘත කර ඔබට අවශ්‍ය සංස්කරණවලින් PHP, වෙබ් සේවාදායකයක් සහ දත්ත සමුදායක් ස්ථාපනය කරන්න.'),
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
            ('මට Docker හෝ අතථ්‍ය යන්ත්‍රයක් අවශ්‍යද?', 'නැත. Lumina සැබෑ PHP, වෙබ් සේවාදායක සහ දත්ත සමුදා වැඩසටහන් ඔබේ පරිගණකයේම ධාවනය කරයි; එබැවින් එය ඉක්මනින් ආරම්භ වී අඩු මතකයක් භාවිත කරයි.'),
            ('මට භාවිත කළ හැක්කේ කුමන PHP සංස්කරණද?', 'Windows මත, ඉක්මන් එකතුවේ ඇති සෑම සංස්කරණයක්ම, එකට. macOS සහ Linux මත, ඔබ Homebrew මගින් ස්ථාපනය කරන සංස්කරණ (උදා: <code>brew install php</code> හෝ <code>php@8.3</code>); Lumina ඒවා තනිවම සොයා ගනී. පරිසරයකට හෝ පැතිකඩකට එක ක්ලික් එකකින් මාරු කරන්න.'),
            ('PostgreSQL සහ MySQL එකට භාවිත කළ හැකිද?', 'ඔව්. PostgreSQL, MySQL හෝ MariaDB සමඟම තමන්ගේම පෝට් එකක ධාවනය වේ.'),
            ('මගේ ව්‍යාපෘති සහ දත්ත සමුදා තබා ඇත්තේ කොහේද?', 'පළමු ආරම්භයේදී ඔබ තෝරන එක් දත්ත ෆෝල්ඩරයක (Windows මත <code>C:\\Lumina</code>, අනෙක් පද්ධතිවල <code>~/Lumina</code>). ඔබ වෙනත් ලෙස නොඉල්ලන්නේ නම් අස්ථාපනය කිරීමේදී එය තබා ගැනේ.'),
            ('එය ආරක්ෂිතද?', 'සේවා සවන් දෙන්නේ ඔබේම පරිගණකයට පමණි. බාගැනීම් තහවුරු කෙරේ, යාවත්කාලීන අත්සන් කර ඇත, පරිපාලක අවසර ඉල්ලන්නේ අවශ්‍ය විට පමණි.'),
        ],
        'footer_tag': 'PHP සඳහා දේශීය සංවර්ධන පරිසරයක්.',
        'footer_license': '<a href="https://github.com/SaanPrasanna/Lumina/blob/master/LICENSE">GNU GPL v3.0</a> යටතේ නිකුත් කර ඇත.',
        'footer_links': [('GitHub', REPO), ('නිකුතු', REPO + '/releases'), ('වෙනස්කම් ලේඛනය', REPO + '/blob/master/CHANGELOG.md'),
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
    ld = {
        '@context': 'https://schema.org',
        '@type': 'SoftwareApplication',
        'name': 'Lumina',
        'description': t['description'],
        'url': url,
        'inLanguage': t['lang'],
        'applicationCategory': 'DeveloperApplication',
        'operatingSystem': 'Windows 10, Windows 11, macOS 11+, Linux',
        'offers': {'@type': 'Offer', 'price': '0', 'priceCurrency': 'USD'},
        'license': 'https://www.gnu.org/licenses/gpl-3.0.html',
        'downloadUrl': LATEST,
        'codeRepository': REPO,
        'image': BASE + 'assets/icon-512.png',
        'author': {'@type': 'Person', 'name': 'Sandun Prasanna', 'url': 'https://github.com/SaanPrasanna'},
    }
    faq_ld = {
        '@context': 'https://schema.org',
        '@type': 'FAQPage',
        'mainEntity': [{'@type': 'Question', 'name': q,
                        'acceptedAnswer': {'@type': 'Answer', 'text': a.replace('<code>', '').replace('</code>', '')}}
                       for q, a in t['faq']],
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
  <meta property="og:type" content="website">
  <meta property="og:site_name" content="Lumina">
  <meta property="og:title" content="{esc(t['og_title'])}">
  <meta property="og:description" content="{esc(t['description'])}">
  <meta property="og:url" content="{url}">
  <meta property="og:image" content="{BASE}assets/{t['og']}">
  <meta property="og:image:width" content="1200">
  <meta property="og:image:height" content="630">
  <meta property="og:locale" content="{'en_US' if t['lang'] == 'en' else 'si_LK'}">
  <meta property="og:locale:alternate" content="{'si_LK' if t['lang'] == 'en' else 'en_US'}">
  <meta name="twitter:card" content="summary_large_image">
  <meta name="twitter:title" content="{esc(t['og_title'])}">
  <meta name="twitter:description" content="{esc(t['description'])}">
  <meta name="twitter:image" content="{BASE}assets/{t['og']}">
  <link rel="preload" href="{up}assets/fonts/manrope-latin.woff2" as="font" type="font/woff2" crossorigin>
{'  <link rel="preload" href="../assets/fonts/noto-sans-sinhala.woff2" as="font" type="font/woff2" crossorigin>' + chr(10) if t['lang'] == 'si' else ''}  <link rel="stylesheet" href="{up}assets/style.css">
  <script type="application/ld+json">{json.dumps(ld, ensure_ascii=False)}</script>
  <script type="application/ld+json">{json.dumps(faq_ld, ensure_ascii=False)}</script>
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
        <p>A local development environment for PHP.</p>
      </div>
      <div class="foot-links">
        <p><a href="{REPO}">GitHub</a> · <a href="{REPO}/releases">Releases</a></p>
        <p>© 2026 Sandun Prasanna. Released under the <a href="{REPO}/blob/master/LICENSE">GNU GPL v3.0</a>.</p>
      </div>
    </div>
  </footer>
</body>
</html>
'''


os.makedirs(ROOT + 'si', exist_ok=True)
for lang, t in T.items():
    out = ROOT + t['path'] + 'index.html'
    open(out, 'w', encoding='utf8', newline='\n').write(page(t))
    print('wrote', out)

open(ROOT + '404.html', 'w', encoding='utf8', newline='\n').write(not_found())
print('wrote', ROOT + '404.html')
