"""Generates the website pages from the texts below, so both languages keep
the same layout: site/index.html (English), site/si/index.html (Sinhala),
site/changelog/ (every release from CHANGELOG.md, with an Atom feed), site/compare/
(Lumina next to Laragon, XAMPP, Herd, MAMP and Local), site/guides/ and the stack
pages (/php/, /laravel/, ...) and
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

# Guide pages (site/guides/<slug>/) and stack pages (site/<slug>/). Each: slug, title and
# description (search results), h1, intro, sections [(heading, html)], faq [(question, html)]
# and related guide slugs. Describe only what Lumina does today, with the app's own names.
GUIDES = [
    {
        'slug': 'php-versions-side-by-side-windows',
        'title': 'Install PHP 7.4 and 8.4 side by side on Windows (one per site)',
        'description': ('How to run several PHP versions on Windows at the same time, PHP 7.4 to 8.5, and give '
                        'each project its own version, without editing Apache or nginx configs by hand.'),
        'h1': 'Run PHP 7.4 and PHP 8.4 side by side on Windows',
        'intro': ('An old shop that needs PHP 7.4 and a new Laravel app on PHP 8.4, on the same computer, at the '
                  'same time. On Windows that usually means two installs and hand-edited server configs. With '
                  'Lumina it takes a few clicks.'),
        'sections': [
            ('1. Install the PHP versions', '''<ol>
<li>Open <strong>Quick Add</strong> and pick <strong>PHP</strong>.</li>
<li>Click <strong>Install</strong> next to each version you need, for example 8.4 and 7.4. Every line lists its three newest patch releases, so you can match a server exactly.</li>
<li>Each download is checked against its published SHA-256 before it is unpacked.</li>
</ol>'''),
            ('2. Choose the version for everything', '''<p>On the <strong>Services</strong> page, the PHP card's <strong>Version</strong> list sets the PHP that every site uses by default. "Newest installed" follows new versions as you install them.</p>'''),
            ('3. Give one project its own version', '''<ol>
<li>Open <strong>Sites</strong> and click <strong>Edit</strong> on the project.</li>
<li>Pick its PHP version, for example 7.4, and save.</li>
<li>The site gets its own PHP pool; every other site keeps the shared one. This works with nginx and Apache.</li>
</ol>'''),
            ('4. Use the same version in the terminal', '''<p>Lumina puts <code>php</code> and <code>composer</code> on PATH (the <strong>PATH</strong> page), following the active version. Check it with:</p>
<pre><code>php -v
composer --version</code></pre>'''),
        ],
        'faq': [
            ('Which PHP versions can I install?', 'PHP 7.4, 8.0, 8.1, 8.2, 8.3, 8.4 and 8.5, each with its three newest patch releases. PHP 7.4 to 8.0 are end of life, so use them only for old projects.'),
            ('Does Xdebug work with every version?', 'On Windows, Quick Add offers Xdebug for every PHP line from 7.4 to 8.5, and the PHP card switches it on in one click.'),
            ('And on macOS or Linux?', 'There PHP comes from Homebrew. Quick Add shows the <code>brew install</code> command for each version line (PHP 8.0 and 7.4 from the shivammathur/php tap), and Lumina finds them on its own.'),
        ],
        'related': ['laravel-https', 'test-domains-windows'],
    },
    {
        'slug': 'laravel-https',
        'title': 'Run Laravel locally with HTTPS at https://app.test',
        'description': ('Set up a Laravel project locally with a real https:// address and a certificate your '
                        'browser trusts, plus MySQL and Mailpit, on Windows, macOS or Linux.'),
        'h1': 'Run Laravel locally with HTTPS',
        'intro': ('Secure cookies, OAuth callbacks and service workers want <code>https://</code>. Lumina gives '
                  'every Laravel project a trusted <code>https://name.test</code> address with no certificate '
                  'commands.'),
        'sections': [
            ('1. Install the stack', '''<ol>
<li>In <strong>Quick Add</strong>, install PHP 8.4 (or 8.3), nginx or Apache, MySQL or MariaDB, and Composer.</li>
<li>Press <strong>Start All</strong> on the Dashboard.</li>
</ol>'''),
            ('2. Create or add the project', '''<p>Create a new one with <strong>QuickApp</strong> &rsaquo; Laravel: Lumina runs Composer, creates the database and writes <code>.env</code>. Or put an existing project in the <code>www</code> folder: a folder named <code>shop</code> answers at <code>http://shop.test</code>, served from its <code>public</code> folder.</p>'''),
            ('3. Turn on HTTPS', '''<ol>
<li>Open <strong>Sites</strong>. Under <strong>HTTPS</strong>, Lumina creates a certificate authority that exists only on your computer.</li>
<li>Trust it once when asked. Browsers then show a padlock for every <code>.test</code> site and for <code>localhost</code>; Firefox is set up too.</li>
<li>Optionally switch on <strong>Force HTTPS</strong> to redirect <code>http://</code> to <code>https://</code>.</li>
</ol>'''),
            ('4. Point Laravel at the new address', '''<p>In <code>.env</code>:</p>
<pre><code>APP_URL=https://shop.test
DB_HOST=127.0.0.1
DB_PORT=3306
DB_USERNAME=root
DB_PASSWORD=</code></pre>'''),
        ],
        'faq': [
            ('Do I need mkcert or OpenSSL?', 'No. Lumina makes and renews the certificates itself.'),
            ('Does it work with Vite in Laravel?', 'Yes. Run <code>npm run dev</code> as usual; the page itself is served at <code>https://shop.test</code>.'),
            ('Where do test emails go?', 'To Mailpit, if you install it: see <a href="../laravel-mailpit/">catch Laravel mail with Mailpit</a>.'),
        ],
        'related': ['laravel-mailpit', 'https-localhost'],
    },
    {
        'slug': 'test-domains-windows',
        'title': 'Local .test domains on Windows: a name for every project',
        'description': ('Give every local project its own address like http://blog.test on Windows, without '
                        'editing the hosts file or Apache virtual hosts by hand.'),
        'h1': 'Local .test domains on Windows',
        'intro': ('<code>localhost/blog</code> breaks absolute URLs, cookies and redirects. A real name per project, '
                  'such as <code>blog.test</code>, behaves like production. <code>.test</code> is reserved for '
                  'exactly this and never exists on the internet.'),
        'sections': [
            ('How Lumina does it', '''<ol>
<li>Every folder in the <code>www</code> folder becomes <code>folder.test</code>.</li>
<li>Lumina writes the web server config for it (nginx or Apache), with the right document root: <code>public</code>, <code>web</code> or the folder itself.</li>
<li>It adds the names to the Windows hosts file, with one administrator prompt when the list changes.</li>
</ol>'''),
            ('Set it up', '''<ol>
<li>Install Lumina and a web server from <strong>Quick Add</strong>, then <strong>Start All</strong>.</li>
<li>Create a folder in <code>www</code>, for example <code>C:\\Lumina\\www\\blog</code>.</li>
<li>Open <code>http://blog.test</code>. The <strong>Sites</strong> page lists every site and says whether the hosts file is up to date.</li>
</ol>'''),
            ('Another suffix', '''<p>Prefer <code>.localhost</code> or your own suffix? Change <strong>Domain suffix</strong> on the Sites page. Avoid <code>.dev</code> and <code>.app</code>: they are real top-level domains that browsers force to HTTPS.</p>'''),
        ],
        'faq': [
            ('Why not .local?', 'It is used by mDNS (Bonjour), which can make lookups slow or fail. .test is reserved for testing.'),
            ('Do phones on my Wi-Fi see these sites?', 'With "Share on my network" on the Sites page, yes, at an address like <code>http://blog.192.168.1.20.nip.io</code>, with a QR code.'),
        ],
        'related': ['https-localhost', 'php-versions-side-by-side-windows'],
    },
    {
        'slug': 'https-localhost',
        'title': 'Trusted HTTPS for localhost and local sites, without warnings',
        'description': ('Get https://localhost and https://project.test with a padlock and no browser warning, '
                        'using a certificate authority that exists only on your computer.'),
        'h1': 'Trusted HTTPS for localhost',
        'intro': ('A self-signed certificate gives you a red warning page in every browser. The fix is a '
                  'certificate authority that your computer trusts and that signs a certificate for each '
                  'local site. Lumina sets that up for you.'),
        'sections': [
            ('Turn it on', '''<ol>
<li>Install Lumina and a web server, then open <strong>Sites</strong>.</li>
<li>Switch on <strong>HTTPS</strong> and trust the certificate authority when asked (one prompt).</li>
<li>Open <code>https://localhost</code> or <code>https://yoursite.test</code>: the padlock is there, in Chrome, Edge, Firefox and Safari.</li>
</ol>'''),
            ('Is it safe?', '''<p>The certificate authority is created on your computer and never leaves it. It is limited to your local names (<code>.test</code> and <code>localhost</code>), so it cannot vouch for real websites. You can regenerate it or remove it from the Sites page at any time.</p>'''),
            ('Node.js and Python dev servers too', '''<p>Vite, Next.js or Django projects in <code>www</code> are served at <code>https://name.test</code> behind the same certificates, and Lumina's dev servers trust them, so calls between your <code>.test</code> sites work.</p>'''),
        ],
        'faq': [
            ('Do I need mkcert?', 'No. Lumina creates, trusts and renews the certificates itself.'),
            ('Does it work offline?', 'Yes. Nothing is requested from the internet.'),
        ],
        'related': ['laravel-https', 'nextjs-django-local-domain'],
    },
    {
        'slug': 'nextjs-django-local-domain',
        'title': 'Run Next.js, Vite or Django at a custom local domain with HTTPS',
        'description': ('Serve Next.js, Nuxt, Vite, SvelteKit, Django, FastAPI or Flask dev servers at '
                        'https://project.test instead of localhost:3000, with hot reload.'),
        'h1': 'Run Next.js, Vite or Django at a local domain',
        'intro': ('<code>localhost:3000</code>, <code>localhost:5173</code>, <code>localhost:8000</code>: ports '
                  'collide and cookies leak between projects. Lumina puts each dev server behind its own '
                  '<code>https://name.test</code> address and keeps hot reload working.'),
        'sections': [
            ('Supported out of the box', '''<p>A folder in <code>www</code> with Vite, Next.js, Nuxt, Astro, SvelteKit or React Router (and no PHP), or with Django, FastAPI or Flask, is recognised and served by its own dev server. Any other command works too: <strong>Edit</strong> &rsaquo; Served by.</p>'''),
            ('Start it', '''<ol>
<li>Install Node.js or Python from <strong>Quick Add</strong>, and a web server.</li>
<li>Put the project in <code>www</code>, for example <code>www\\storefront</code>.</li>
<li>On <strong>Sites</strong>, press <strong>Start</strong> on its row. Lumina runs the dev server on a fixed port and shows its log.</li>
<li>Open <code>https://storefront.test</code>. Saving a file reloads the page as usual.</li>
</ol>'''),
            ('The right runtime version', '''<p>A project's <code>.nvmrc</code>, <code>.node-version</code>, <code>.python-version</code> or <code>.tool-versions</code> picks the Node.js or Python version its dev server runs with, from the versions installed in Quick Add.</p>'''),
        ],
        'faq': [
            ('Can the dev server start with Start All?', 'Yes, switch on autostart for it in Edit.'),
            ('Does it work with pnpm, Bun or uv?', 'Yes. Bun, Deno, uv and pnpm are in Quick Add and on PATH in every terminal.'),
        ],
        'related': ['https-localhost', 'test-domains-windows'],
    },
    {
        'slug': 'laravel-mailpit',
        'title': 'Catch Laravel mail locally with Mailpit (no real emails sent)',
        'description': ('Send every email from your local Laravel or PHP app to a local Mailpit inbox instead of '
                        'real people, with the .env settings to use.'),
        'h1': 'Catch Laravel mail locally with Mailpit',
        'intro': ('Testing password resets and order mails should never reach a real customer. Mailpit is a '
                  'local mail server with a web inbox: everything your apps send lands there.'),
        'sections': [
            ('Install and start Mailpit', '''<ol>
<li>Open <strong>Quick Add</strong> and install <strong>Mailpit</strong>.</li>
<li>Start it from its card on the <strong>Services</strong> page, or with Start All.</li>
<li>Open the inbox from the same card or from the tray.</li>
</ol>'''),
            ('Point Laravel at it', '''<p>New Laravel projects from QuickApp are set up already. For an existing one, in <code>.env</code>:</p>
<pre><code>MAIL_MAILER=smtp
MAIL_HOST=127.0.0.1
MAIL_PORT=1025
MAIL_USERNAME=null
MAIL_PASSWORD=null</code></pre>
<p>Any user name and password are accepted.</p>'''),
            ('Plain PHP and WordPress', '''<p>PHP's <code>mail()</code> goes to Mailpit too while it runs, so WordPress and other PHP apps need no plugin. Switch that off in Settings &rsaquo; Mail.</p>'''),
        ],
        'faq': [
            ('Can mail still reach the internet?', 'No. Mailpit keeps every message locally; nothing is delivered.'),
            ('Which port is the inbox on?', 'SMTP on 1025 and the web inbox on 8025, both on your computer only.'),
        ],
        'related': ['laravel-https', 'php-versions-side-by-side-windows'],
    },
]

FRAMEWORKS = [
    {
        'slug': 'php',
        'title': 'Local PHP development environment for Windows, macOS and Linux',
        'description': ('A free local PHP environment: PHP 7.4 to 8.5 side by side, nginx or Apache, MySQL, '
                        'PostgreSQL and Redis, Composer and Xdebug, with .test sites and trusted HTTPS.'),
        'h1': 'Local PHP development, set up in minutes',
        'intro': ('Lumina runs real PHP, a web server and a database on your own computer, with no Docker and no '
                  'virtual machine. Every project gets its own address and the PHP version it needs.'),
        'sections': [
            ('What you get', '''<ul>
<li>PHP 7.4 to 8.5 as FastCGI, side by side, one version per site if you like.</li>
<li>nginx or Apache, MySQL or MariaDB, PostgreSQL and Redis, started with one click.</li>
<li><code>php</code> and <code>composer</code> in every terminal, following the active version.</li>
<li>Xdebug in one switch, Mailpit for outgoing mail, phpMyAdmin and Adminer for databases.</li>
<li>Every folder in <code>www</code> at <code>https://folder.test</code> with a trusted certificate.</li>
</ul>'''),
            ('Get started', '''<ol>
<li>Download and install Lumina.</li>
<li>In <strong>Quick Add</strong>, install PHP, a web server and a database.</li>
<li>Press <strong>Start All</strong> and open <code>http://localhost</code>: the welcome page lists every database login.</li>
</ol>'''),
        ],
        'faq': [
            ('Is Lumina free?', 'Yes, open source under the GNU GPL v3.0, with no paid edition.'),
            ('Can I use Apache .htaccess files?', 'Yes, choose Apache as the web server and .htaccess works as on most shared hosting.'),
        ],
        'related': ['php-versions-side-by-side-windows', 'test-domains-windows', 'https-localhost'],
    },
    {
        'slug': 'laravel',
        'title': 'Local Laravel development environment, free (Herd and Valet alternative)',
        'description': ('Run Laravel locally with PHP 8.4, nginx, MySQL or PostgreSQL, Redis and Mailpit, at '
                        'https://app.test, on Windows, macOS and Linux. Free and open source.'),
        'h1': 'Laravel, locally, with everything included',
        'intro': ('Everything a Laravel app expects, on your own computer: PHP, Composer, a database, Redis for '
                  'queues and cache, and Mailpit for mail, all free.'),
        'sections': [
            ('A new project in one click', '''<p><strong>QuickApp</strong> &rsaquo; Laravel creates the project with Composer, makes its database, writes <code>.env</code> (database, mail to Mailpit) and opens it at <code>https://name.test</code>.</p>'''),
            ('What Laravel gets', '''<ul>
<li>PHP 8.2 to 8.5 with the extensions Laravel needs, and Composer on PATH.</li>
<li>MySQL, MariaDB or PostgreSQL, and Redis for cache, queues and sessions.</li>
<li>Mailpit for every mail, Xdebug in one switch, database snapshots before a risky migration.</li>
<li>A public HTTPS link through Cloudflare Tunnel or ngrok for webhooks and demos.</li>
</ul>'''),
        ],
        'faq': [
            ('Does it replace Laravel Herd or Valet?', 'It covers the same ground, including databases, Mailpit and Xdebug, for free, and runs on Linux too. See <a href="../compare/herd/">Lumina vs Herd</a>.'),
            ('Does Vite work?', 'Yes. Run <code>npm run dev</code>; Node.js comes from Quick Add.'),
        ],
        'related': ['laravel-https', 'laravel-mailpit', 'php-versions-side-by-side-windows'],
    },
    {
        'slug': 'wordpress',
        'title': 'Local WordPress development environment, free (Local and XAMPP alternative)',
        'description': ('Run WordPress locally at https://site.test with PHP, MySQL, Apache or nginx and Mailpit, '
                        'on Windows, macOS and Linux. Free and open source.'),
        'h1': 'WordPress on your own computer',
        'intro': ('Build themes and plugins against a real WordPress install, with a proper address, HTTPS and '
                  'every outgoing mail caught locally.'),
        'sections': [
            ('A new site in one click', '''<p><strong>QuickApp</strong> &rsaquo; WordPress downloads WordPress and creates its database, so the site is ready at <code>https://name.test</code>.</p>'''),
            ('What WordPress gets', '''<ul>
<li>PHP 7.4 to 8.5 per site, so old and new sites run side by side.</li>
<li>Apache (with <code>.htaccess</code> permalinks) or nginx, MySQL or MariaDB.</li>
<li>Mail from <code>wp_mail()</code> lands in Mailpit, no SMTP plugin needed.</li>
<li>phpMyAdmin and Adminer, database snapshots, and a public link to show a client.</li>
</ul>'''),
        ],
        'faq': [
            ('Can I move an existing site in?', 'Yes. Copy its folder into <code>www</code>, import the database with phpMyAdmin, and update <code>siteurl</code> and <code>home</code> (for example with WP-CLI <code>search-replace</code>).'),
            ('Is it an alternative to Local?', 'Yes, and it is not limited to WordPress. See <a href="../compare/local/">Lumina vs Local</a>.'),
        ],
        'related': ['laravel-mailpit', 'test-domains-windows', 'https-localhost'],
    },
    {
        'slug': 'nodejs',
        'title': 'Local Node.js development: Next.js, Vite and Nuxt at https://app.test',
        'description': ('Install Node.js versions side by side and serve Next.js, Vite, Nuxt, Astro or SvelteKit '
                        'dev servers at their own https://name.test address, with hot reload.'),
        'h1': 'Node.js projects at their own local domain',
        'intro': ('Several Node.js versions, pnpm, Bun and Deno, and every front-end project at '
                  '<code>https://name.test</code> instead of a port number.'),
        'sections': [
            ('What you get', '''<ul>
<li>Node.js 18 to 26 from Quick Add, with npm, npx and pnpm; Bun and Deno too.</li>
<li>The version from <code>.nvmrc</code> or <code>.node-version</code> per project.</li>
<li>Vite, Next.js, Nuxt, Astro, SvelteKit and React Router dev servers at <code>https://name.test</code>, with hot reload.</li>
<li>Next to PHP, MySQL, PostgreSQL, Redis and MongoDB when your app needs a backend.</li>
</ul>'''),
            ('Get started', '''<ol>
<li>Install Lumina, then Node.js and a web server from <strong>Quick Add</strong>.</li>
<li>Put the project in <code>www</code> and press <strong>Start</strong> on its row in <strong>Sites</strong>.</li>
</ol>'''),
        ],
        'faq': [
            ('Does it replace nvm?', 'For Lumina\'s terminals and dev servers, yes: install versions in Quick Add and pick one per project with .nvmrc.'),
        ],
        'related': ['nextjs-django-local-domain', 'https-localhost'],
    },
    {
        'slug': 'python',
        'title': 'Local Python development: Django, FastAPI and Flask at https://app.test',
        'description': ('Install Python versions side by side and serve Django, FastAPI or Flask dev servers at '
                        'their own https://name.test address, with PostgreSQL and Redis next to them.'),
        'h1': 'Python projects at their own local domain',
        'intro': ('Python 3.10 to 3.14, uv, and Django, FastAPI or Flask served at <code>https://name.test</code>, '
                  'next to PostgreSQL, MySQL and Redis.'),
        'sections': [
            ('What you get', '''<ul>
<li>Python 3.10 to 3.14 from Quick Add, with pip and venv; uv too.</li>
<li>The version from <code>.python-version</code> or <code>.tool-versions</code> per project.</li>
<li>Django, FastAPI and Flask dev servers at <code>https://name.test</code>, with auto-reload.</li>
<li>PostgreSQL, MySQL, Redis and Mailpit with one click.</li>
</ul>'''),
            ('Get started', '''<ol>
<li>Install Lumina, then Python and a web server from <strong>Quick Add</strong>.</li>
<li>Put the project in <code>www</code> and press <strong>Start</strong> on its row in <strong>Sites</strong>.</li>
</ol>'''),
        ],
        'faq': [
            ('Does it replace pyenv?', 'For Lumina\'s terminals and dev servers, yes: install versions in Quick Add and pick one per project with .python-version.'),
        ],
        'related': ['nextjs-django-local-domain', 'https-localhost'],
    },
]

T = {
    'en': {
        'lang': 'en', 'dir': '', 'path': '', 'og': 'og-en.png',
        'title': 'Lumina – free Laragon and XAMPP alternative for PHP, Laravel and Node.js',
        'description': ('Lumina is a free, open-source local development environment for web developers, '
                        'and an alternative to Laragon, XAMPP, Herd and MAMP. Run PHP 7.4 to 8.5, Node.js and Python '
                        'with nginx or Apache, MySQL, PostgreSQL and Redis, with .test sites and trusted HTTPS, '
                        'on Windows, macOS and Linux.'),
        'og_title': 'Lumina – local development, one click away',
        'og_alt': 'Lumina – local development, one click away. Free and open source for Windows, macOS and Linux, '
                  'with PHP, Node.js, Python, MySQL and PostgreSQL.',
        'app_kind': 'Local development environment',
        'nav_features': 'Features', 'nav_download': 'Download', 'nav_faq': 'FAQ', 'nav_compare': 'Compare', 'nav_guides': 'Guides', 'nav_changelog': 'Changelog',
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
        'shots_title': 'See it in action',
        'shots_lead': 'The real app, with a few example projects.',
        'shots': [
            ('dashboard', 'Lumina dashboard: PHP, nginx, MySQL, PostgreSQL, Redis and Mailpit running, each with its version and port', 'Every service at a glance'),
            ('sites', 'Lumina Sites page: automatic .test sites and trusted local HTTPS from a certificate authority on your computer', 'A .test address and HTTPS for every folder'),
            ('quick-add', 'Lumina Quick Add: PHP 8.5 to 7.4 to install side by side, each download checked against its SHA-256', 'Any version, one click'),
            ('services', 'Lumina Services page: PHP version, Xdebug switch, nginx or Apache, MySQL or MariaDB settings', 'Every server under control'),
        ],
        'made_title': 'Made for your stack',
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
            ('Which PHP versions can I use?', 'PHP 7.4 to 8.5, side by side, and each site can use its own. On Windows, Quick Add installs them, with the three newest patch releases of every version line. On macOS and Linux they come from Homebrew: Quick Add shows the <code>brew install</code> command for each version line, and Lumina finds them on its own.'),
            ('Is Lumina an alternative to Laragon, XAMPP or Herd?', 'Yes. Lumina covers what they do (PHP, a web server, databases, <code>.test</code> sites and HTTPS) for free, on Windows, macOS and Linux, and adds Node.js and Python dev servers. See the side-by-side <a href="compare/">comparisons</a>.'),
            ('Can I use PostgreSQL and MySQL together?', 'Yes. PostgreSQL runs next to MySQL or MariaDB, each on its own port.'),
            ('Where are my projects and databases kept?', 'In one data folder you choose on first start (<code>C:\\Lumina</code> on Windows, <code>~/Lumina</code> elsewhere). Uninstalling keeps it unless you ask otherwise.'),
            ('Is it safe?', 'Services listen only on your own computer. Downloads are verified, updates are signed, and administrator rights are asked for only when they are needed.'),
        ],
        'footer_tag': 'A local development environment for web developers.',
        'footer_license': 'Released under the <a href="https://github.com/SaanPrasanna/Lumina/blob/master/LICENSE">GNU GPL v3.0</a>.',
        'footer_links': [('GitHub', REPO), ('Releases', REPO + '/releases'), ('Changelog', BASE + 'changelog/'),
                         ('Compare', BASE + 'compare/'), ('Guides', BASE + 'guides/'),
                         ('Report a bug', REPO + '/issues/new/choose'), ('Security', REPO + '/blob/master/SECURITY.md')],
        'mock_running': 'Running', 'mock_stopped': 'Stopped', 'mock_start_all': 'Stop All',
        'mock_services': 'Services',
    },
    'si': {
        'lang': 'si', 'dir': '', 'path': 'si/', 'og': 'og-si.png',
        'title': 'Lumina – නොමිලේ දේශීය සංවර්ධන පරිසරය, Laragon සහ XAMPP සඳහා විකල්පයක්',
        'description': ('Lumina යනු වෙබ් සංවර්ධකයින් සඳහා නොමිලේ ලබා ගත හැකි විවෘත මූලාශ්‍ර දේශීය සංවර්ධන පරිසරයකි. '
                        'nginx හෝ Apache, MySQL, PostgreSQL, Redis සහ තවත් දේ සමඟ PHP, Node.js සහ Python ධාවනය කරන්න; ස්වයංක්‍රීය '
                        '.test අඩවි සහ විශ්වාසනීය HTTPS සමඟ, Windows, macOS සහ Linux මත. Laragon, XAMPP, Herd සහ MAMP සඳහා විකල්පයකි.'),
        'og_title': 'Lumina – ඔබේ සංවර්ධන පරිසරය, එක ක්ලික් එකකින්',
        'og_alt': 'Lumina – ඔබේ සංවර්ධන පරිසරය, එක ක්ලික් එකකින්. Windows, macOS සහ Linux සඳහා නොමිලේ සහ විවෘත මූලාශ්‍ර; '
                  'PHP, Node.js, Python, MySQL සහ PostgreSQL සමඟ.',
        'app_kind': 'දේශීය සංවර්ධන පරිසරය',
        'nav_features': 'විශේෂාංග', 'nav_download': 'බාගන්න', 'nav_faq': 'ප්‍රශ්න', 'nav_compare': 'සසඳන්න', 'nav_guides': 'මාර්ගෝපදේශ', 'nav_changelog': 'වෙනස්කම්',
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
        'shots_title': 'ක්‍රියාත්මකව බලන්න',
        'shots_lead': 'සැබෑ යෙදුම, උදාහරණ ව්‍යාපෘති කිහිපයක් සමඟ.',
        'shots': [
            ('dashboard', 'Lumina උපකරණ පුවරුව: PHP, nginx, MySQL, PostgreSQL, Redis සහ Mailpit ධාවනය වේ', 'සියලු සේවා එක බැල්මකින්'),
            ('sites', 'Lumina අඩවි පිටුව: ස්වයංක්‍රීය .test අඩවි සහ විශ්වාසනීය දේශීය HTTPS', 'සෑම ෆෝල්ඩරයකටම .test ලිපිනයක් සහ HTTPS'),
            ('quick-add', 'Lumina ඉක්මන් එකතුව: PHP 8.5 සිට 7.4 දක්වා එකට ස්ථාපනය කරන්න', 'ඕනෑම සංස්කරණයක්, එක ක්ලික් එකකින්'),
            ('services', 'Lumina සේවා පිටුව: PHP සංස්කරණය, Xdebug, nginx හෝ Apache, MySQL හෝ MariaDB', 'සෑම සේවාදායකයක්ම ඔබේ පාලනයේ'),
        ],
        'made_title': 'ඔබේ තාක්ෂණයට ගැළපේ',
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
            ('මට භාවිත කළ හැක්කේ කුමන PHP සංස්කරණද?', 'PHP 7.4 සිට 8.5 දක්වා, එකට; සෑම අඩවියකටම තමන්ගේම සංස්කරණයක් භාවිත කළ හැක. Windows මත ඉක්මන් එකතුව ඒවා ස්ථාපනය කරයි; සෑම සංස්කරණ පෙළකම නවතම patch නිකුතු තුන සමඟ. macOS සහ Linux මත ඒවා Homebrew මගින් ලැබේ: ඉක්මන් එකතුව සෑම සංස්කරණ පෙළකටම <code>brew install</code> විධානය පෙන්වයි, Lumina ඒවා තනිවම සොයා ගනී.'),
            ('Lumina යනු Laragon, XAMPP හෝ Herd සඳහා විකල්පයක්ද?', 'ඔව්. ඒවා කරන දේ (PHP, වෙබ් සේවාදායකයක්, දත්ත සමුදා, <code>.test</code> අඩවි සහ HTTPS) Lumina නොමිලේ, Windows, macOS සහ Linux මත කරයි; Node.js සහ Python dev සේවාදායක ද එක් කරයි. <a href="../compare/" hreflang="en">සංසන්දන</a> බලන්න (ඉංග්‍රීසියෙන්).'),
            ('PostgreSQL සහ MySQL එකට භාවිත කළ හැකිද?', 'ඔව්. PostgreSQL, MySQL හෝ MariaDB සමඟම තමන්ගේම පෝට් එකක ධාවනය වේ.'),
            ('මගේ ව්‍යාපෘති සහ දත්ත සමුදා තබා ඇත්තේ කොහේද?', 'පළමු ආරම්භයේදී ඔබ තෝරන එක් දත්ත ෆෝල්ඩරයක (Windows මත <code>C:\\Lumina</code>, අනෙක් පද්ධතිවල <code>~/Lumina</code>). ඔබ වෙනත් ලෙස නොඉල්ලන්නේ නම් අස්ථාපනය කිරීමේදී එය තබා ගැනේ.'),
            ('එය ආරක්ෂිතද?', 'සේවා සවන් දෙන්නේ ඔබේම පරිගණකයට පමණි. බාගැනීම් තහවුරු කෙරේ, යාවත්කාලීන අත්සන් කර ඇත, පරිපාලක අවසර ඉල්ලන්නේ අවශ්‍ය විට පමණි.'),
        ],
        'footer_tag': 'වෙබ් සංවර්ධකයින් සඳහා දේශීය සංවර්ධන පරිසරයක්.',
        'footer_license': '<a href="https://github.com/SaanPrasanna/Lumina/blob/master/LICENSE">GNU GPL v3.0</a> යටතේ නිකුත් කර ඇත.',
        'footer_links': [('GitHub', REPO), ('නිකුතු', REPO + '/releases'), ('වෙනස්කම් ලේඛනය', BASE + 'changelog/'),
                         ('සසඳන්න', BASE + 'compare/'), ('මාර්ගෝපදේශ', BASE + 'guides/'),
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
                'screenshot': [{'@type': 'ImageObject', 'url': BASE + f'assets/screens/{n}@2x.webp',
                                'width': 2560, 'height': 1600, 'caption': alt} for n, alt, _ in t['shots']],
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

    shots = '\n'.join(
        f'''        <figure class="shot">
          <img src="{up}assets/screens/{name}.webp" srcset="{up}assets/screens/{name}.webp 1x, {up}assets/screens/{name}@2x.webp 2x"
               width="1280" height="800" alt="{esc(alt)}"{' loading="lazy"' if n > 1 else ''} decoding="async">
          <figcaption>{esc(cap)}</figcaption>
        </figure>''' for n, (name, alt, cap) in enumerate(t['shots']))
    made = '\n'.join(
        f'        <li><a href="{up}{f["slug"]}/"{" hreflang=\"en\"" if t["lang"] != "en" else ""}>{esc(f["h1"])}</a></li>'
        for f in FRAMEWORKS)
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
        <a href="{up}compare/"{' hreflang="en"' if t['lang'] != 'en' else ''}>{esc(t['nav_compare'])}</a>
        <a href="{up}guides/"{' hreflang="en"' if t['lang'] != 'en' else ''}>{esc(t['nav_guides'])}</a>
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

    <section class="section wrap" aria-labelledby="shots-title">
      <h2 id="shots-title">{esc(t['shots_title'])}</h2>
      <p class="section-lead">{esc(t['shots_lead'])}</p>
      <div class="shots">
{shots}
      </div>
    </section>

    <section class="section wrap" aria-labelledby="made-title">
      <h2 id="made-title">{esc(t['made_title'])}</h2>
      <ul class="made">
{made}
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


# Comparison pages: site/compare/ (all tools side by side) and one page per tool.
# A cell starts with + (has it), - (does not), ~ (only in a paid edition or partly);
# other text is shown as it is, and '' means not confirmed. Keep the facts to what
# each product's own website says, and update CHECKED when they are checked again.
CHECKED = 'October 2026'

ROWS = [
    ('price', 'Price'),
    ('source', 'Source code'),
    ('windows', 'Windows'),
    ('macos', 'macOS'),
    ('linux', 'Linux'),
    ('php', 'PHP versions'),
    ('web', 'Web servers'),
    ('db', 'Databases'),
    ('sites', 'Automatic local domains'),
    ('https', 'Trusted local HTTPS'),
    ('node', 'Node.js and Python projects'),
    ('mail', 'Mail catcher'),
    ('xdebug', 'Xdebug'),
    ('share', 'Share a site publicly'),
    ('apps', 'One-click app installers'),
]

LUMINA = {
    'name': 'Lumina',
    'price': '+Free, no paid edition',
    'source': '+Open source (GPL-3.0)',
    'windows': '+Windows 10 and 11 (x64, ARM64)',
    'macos': '+macOS 11 or later',
    'linux': '+AppImage, .deb, .rpm',
    'php': '+PHP 7.4 to 8.5 side by side, one per site',
    'web': '+nginx or Apache',
    'db': '+MySQL, MariaDB, PostgreSQL, MongoDB, Redis, Valkey',
    'sites': '+Every folder becomes folder.test',
    'https': '+From a certificate authority on your computer',
    'node': '+Vite, Next.js, Nuxt, Django, FastAPI and more at folder.test',
    'mail': '+Mailpit',
    'xdebug': '+One switch',
    'share': '+Cloudflare Tunnel or ngrok, and phones on your Wi-Fi',
    'apps': '+Laravel, WordPress, Symfony, Drupal and more',
}

COMPETITORS = [
    {
        'slug': 'laragon', 'name': 'Laragon', 'site': 'https://laragon.org/',
        'title': 'Lumina vs Laragon: a free, open-source Laragon alternative',
        'description': ('Looking for a Laragon alternative? Lumina is free and open source, runs on Windows, '
                        'macOS and Linux, and gives every project a .test address with trusted HTTPS, '
                        'like Laragon, plus Node.js and Python dev servers.'),
        'intro': ('Laragon made local PHP development on Windows fast and simple: every folder a <code>.test</code> '
                  'site, Apache or nginx, MySQL, all from the tray. Recent versions need a paid license, and it '
                  'still runs on Windows only. Lumina works the same way, stays free and open source, and runs '
                  'on Windows, macOS and Linux.'),
        'cells': {
            'price': '~Paid license for recent versions',
            'source': '-Closed source',
            'windows': '+Windows',
            'macos': '-No',
            'linux': '-No',
            'php': 'Several versions, one active at a time',
            'web': '+Apache or nginx',
            'db': 'MySQL and Redis; others added by hand',
            'sites': '+folder.test',
            'https': '+Yes',
            'node': '-Node.js and Python on PATH, no dev-server domains',
            'mail': '+Mail catcher',
            'xdebug': '~Added by hand',
            'share': '+ngrok',
            'apps': '+Quick app',
        },
        'why': [
            ('Free, for good', 'Lumina is GPL-3.0 open source with no paid edition, for personal and commercial work alike.'),
            ('The same habits', 'Folders in <code>www</code> become <code>folder.test</code>, Start All from the tray, a menu per service: it feels familiar from the first minute.'),
            ('Beyond Windows and PHP', 'The same stack on macOS and Linux, and Vite, Next.js or Django projects get their own <code>.test</code> address too.'),
        ],
        'move': [
            'Quit Laragon (Stop All, then Exit), so ports 80, 443 and 3306 are free.',
            'Install Lumina and add PHP, a web server and MySQL or MariaDB from Quick Add.',
            "Copy your project folders from <code>C:\\laragon\\www</code> to Lumina's <code>www</code> folder (<code>C:\\Lumina\\www</code> by default). They keep their <code>folder.test</code> addresses.",
            'Export your databases from Laragon (HeidiSQL, phpMyAdmin or <code>mysqldump --all-databases</code>) and import them in Lumina with phpMyAdmin, Adminer or <code>mysql &lt; dump.sql</code>.',
        ],
        'faq': [
            ('Is Lumina free like the old Laragon?', 'Yes. Lumina is open source under the GNU GPL v3.0. There is no license key, no paid edition and no limit on commercial use.'),
            ('Can I run Laragon and Lumina at the same time?', "Not with their default ports: both want ports 80, 443 and 3306. Stop one before you start the other, or change the ports in Lumina's Settings."),
            ('Do my Laragon projects work in Lumina?', "Yes. They are ordinary folders. Copy them into Lumina's www folder and they get the same folder.test address; only the databases need to be exported and imported."),
        ],
    },
    {
        'slug': 'xampp', 'name': 'XAMPP', 'site': 'https://www.apachefriends.org/',
        'title': 'Lumina vs XAMPP: a modern XAMPP alternative with .test sites and HTTPS',
        'description': ('Looking for an XAMPP alternative? Lumina is free and open source, with PHP 7.4 to 8.5 '
                        'side by side, nginx or Apache, MySQL and PostgreSQL, automatic .test sites and trusted '
                        'HTTPS, on Windows, macOS and Linux.'),
        'intro': ('XAMPP is where many developers started: Apache, MariaDB and PHP in one installer. It is free, '
                  'but every project lives at <code>localhost/folder</code>, there is one PHP version per install '
                  'and HTTPS comes with a browser warning. Lumina keeps the one-installer simplicity and adds '
                  'what modern projects need.'),
        'cells': {
            'price': '+Free',
            'source': '+Open source',
            'windows': '+Windows',
            'macos': '+macOS',
            'linux': '+Linux',
            'php': '-One version per install',
            'web': 'Apache',
            'db': 'MariaDB',
            'sites': '-localhost/folder; virtual hosts edited by hand',
            'https': '-Self-signed (browser warning)',
            'node': '-No',
            'mail': '-No',
            'xdebug': '~Added by hand',
            'share': '-No',
            'apps': '-No',
        },
        'why': [
            ('A real address per project', 'Every folder becomes <code>https://folder.test</code>, with a padlock your browser trusts. No <code>httpd-vhosts.conf</code> to edit.'),
            ('Any PHP version, per site', 'Keep an old project on PHP 7.4 and a new one on 8.5, side by side, without a second install.'),
            ('More than Apache and MariaDB', 'nginx, MySQL, PostgreSQL, Redis, MongoDB and Mailpit, plus Node.js and Python, each a click away in Quick Add.'),
        ],
        'move': [
            'Stop Apache and MySQL in the XAMPP Control Panel, so ports 80 and 3306 are free.',
            'Install Lumina and add PHP, Apache or nginx, and MariaDB or MySQL from Quick Add.',
            "Copy your projects from <code>C:\\xampp\\htdocs</code> to Lumina's <code>www</code> folder. <code>localhost/shop</code> becomes <code>https://shop.test</code>; update the site URL in your app's settings (<code>APP_URL</code>, <code>siteurl</code>).",
            "Export each database with XAMPP's phpMyAdmin and import it with phpMyAdmin or Adminer in Lumina.",
        ],
        'faq': [
            ('Is Lumina free like XAMPP?', 'Yes. Both are free and open source. Lumina is licensed under the GNU GPL v3.0.'),
            ('Can I keep using Apache?', 'Yes. Choose Apache or nginx; Lumina writes the configuration for every site.'),
            ('Will my WordPress site work after the move?', "Yes. Update <code>siteurl</code> and <code>home</code> to the new <code>https://folder.test</code> address, for example with WP-CLI's <code>search-replace</code>."),
        ],
    },
    {
        'slug': 'herd', 'name': 'Laravel Herd', 'site': 'https://herd.laravel.com/',
        'title': 'Lumina vs Laravel Herd: a free Herd alternative with databases included',
        'description': ('Looking for a Laravel Herd alternative? Lumina is free and open source, includes MySQL, '
                        'PostgreSQL, Redis, Mailpit and Xdebug at no cost, and runs on Windows, macOS and Linux.'),
        'intro': ('Laravel Herd is a fast, polished way to run PHP and nginx for Laravel. Its databases, mail '
                  'catcher and Xdebug are part of Herd Pro, a paid subscription, and there is no Linux version. '
                  'Lumina includes all of them for free, on all three systems, and is open source.'),
        'cells': {
            'price': '~Free; databases, mail and Xdebug need Herd Pro (paid)',
            'source': '-Closed source',
            'windows': '+Windows',
            'macos': '+macOS',
            'linux': '-No',
            'php': '+Several versions, one per site',
            'web': 'nginx',
            'db': '~With Herd Pro (paid)',
            'sites': '+folder.test',
            'https': '+Yes',
            'node': 'Node.js versions included',
            'mail': '~Herd Pro (paid)',
            'xdebug': '~Herd Pro (paid)',
            'share': '+Expose',
            'apps': 'New Laravel projects',
        },
        'why': [
            ('Everything included, for free', 'MySQL, MariaDB, PostgreSQL, Redis, Mailpit, Xdebug and database snapshots, with no subscription.'),
            ('Linux too', 'The same app and the same workflow on Windows, macOS and Linux, so a whole team can use it.'),
            ('Not only Laravel', 'WordPress, Symfony, Drupal and plain PHP, plus Next.js, Nuxt and Django projects behind their own dev server.'),
        ],
        'move': [
            'Quit Herd (and stop its services), so ports 80 and 443 and the <code>.test</code> domains are free.',
            'Install Lumina and add PHP, nginx and the databases you used from Quick Add.',
            "Move or copy your projects from the Herd folder to Lumina's <code>www</code> folder. They keep their <code>folder.test</code> addresses.",
            'Export your databases (TablePlus, <code>mysqldump</code> or <code>pg_dump</code>) and import them in Lumina.',
        ],
        'faq': [
            ('Does Lumina work with Laravel?', 'Yes. QuickApp creates a new Laravel project with its database in one click, and new projects send mail to Mailpit out of the box.'),
            ('Is there a Pro version of Lumina?', 'No. Every feature is free and open source under the GNU GPL v3.0.'),
            ('Does Lumina run on Linux?', 'Yes: AppImage, .deb and .rpm for x64.'),
        ],
    },
    {
        'slug': 'mamp', 'name': 'MAMP', 'site': 'https://www.mamp.info/',
        'title': 'Lumina vs MAMP: a free, open-source MAMP PRO alternative',
        'description': ('Looking for a MAMP or MAMP PRO alternative? Lumina is free and open source, with .test '
                        'sites, trusted HTTPS, PHP 7.4 to 8.5 per site and MySQL or PostgreSQL, on Windows, macOS '
                        'and Linux.'),
        'intro': ('MAMP brought Apache, MySQL and PHP to the Mac in one app. Named hosts, HTTPS and most extras '
                  'are part of MAMP PRO, which is paid. Lumina gives you those for free, on Windows, macOS and '
                  'Linux, and is open source.'),
        'cells': {
            'price': '~Free MAMP; MAMP PRO is paid',
            'source': '-Closed source',
            'windows': '+Windows',
            'macos': '+macOS',
            'linux': '-No',
            'php': '~Several versions; one per host in MAMP PRO',
            'web': '+Apache or nginx',
            'db': 'MySQL; more in MAMP PRO',
            'sites': '~Named hosts in MAMP PRO',
            'https': '~In MAMP PRO',
            'node': '',
            'mail': '',
            'xdebug': '+Included',
            'share': '',
            'apps': '~In MAMP PRO',
        },
        'why': [
            ('PRO features, free', 'Named <code>.test</code> sites, trusted HTTPS and a PHP version per site, with no license to buy.'),
            ('Standard ports', 'Sites answer at <code>https://folder.test</code> on ports 80 and 443, not <code>localhost:8888</code>.'),
            ('A wider stack', 'PostgreSQL, Redis, MongoDB, Mailpit, Node.js and Python next to PHP and MySQL.'),
        ],
        'move': [
            'Stop the servers in MAMP. Its default ports (8888, 8889) do not clash with Lumina, but its Apache on port 80 would.',
            'Install Lumina and add PHP, Apache or nginx, and MySQL from Quick Add.',
            "Copy your projects from MAMP's <code>htdocs</code> folder to Lumina's <code>www</code> folder; each becomes <code>https://folder.test</code>.",
            "Export your databases with MAMP's phpMyAdmin and import them with phpMyAdmin or Adminer in Lumina.",
        ],
        'faq': [
            ('Is Lumina free like MAMP?', 'Yes, and unlike MAMP PRO, every feature is free: named sites, HTTPS and a PHP version per site included.'),
            ('Does Lumina run on Apple silicon?', 'Yes. The macOS download is a universal app for Apple silicon and Intel Macs.'),
            ('Can I keep using Apache?', 'Yes. Choose Apache or nginx; Lumina writes the configuration for every site.'),
        ],
    },
]

COMPETITORS.append({
    'slug': 'local', 'name': 'Local', 'site': 'https://localwp.com/',
    'title': 'Lumina vs Local (LocalWP): a free alternative beyond WordPress',
    'description': ('Looking for a Local by Flywheel alternative? Lumina is free and open source, runs WordPress '
                    'and any other PHP, Node.js or Python project, with .test sites and trusted HTTPS.'),
    'intro': ('Local (formerly Local by Flywheel) is a friendly way to run WordPress sites. It is built for '
              'WordPress only. Lumina keeps the one-click WordPress setup and also runs Laravel, Symfony, plain '
              'PHP, Next.js, Django and more, from one open-source app.'),
    'cells': {
        'price': '+Free',
        'source': '-Closed source',
        'windows': '+Windows',
        'macos': '+macOS',
        'linux': '+Linux',
        'php': '+Several versions, one per site',
        'web': '+nginx or Apache',
        'db': 'MySQL',
        'sites': '+site.local',
        'https': '+Yes',
        'node': '-No',
        'mail': '+Yes',
        'xdebug': '+Add-on',
        'share': '+Live Links',
        'apps': '~WordPress only',
    },
    'why': [
        ('Not only WordPress', 'Laravel, Symfony, Drupal and plain PHP, plus Next.js, Vite and Django projects, in the same app.'),
        ('Your folders, your tools', 'Projects are ordinary folders in <code>www</code>; open them in any editor, terminal or Git client.'),
        ('Open source', 'GPL-3.0, no account to sign in to, nothing sent anywhere.'),
    ],
    'move': [
        'In Local, export each site (right-click, Export) or copy its <code>app/public</code> folder, and export its database.',
        'Install Lumina and add PHP, a web server and MySQL from Quick Add.',
        "Copy the site into Lumina's <code>www</code> folder and import the database with phpMyAdmin or Adminer.",
        'Update <code>siteurl</code> and <code>home</code> to <code>https://folder.test</code>, for example with WP-CLI <code>search-replace</code>.',
    ],
    'faq': [
        ('Is Lumina good for WordPress?', 'Yes. QuickApp creates a WordPress site with its database in one click, and every mail from <code>wp_mail()</code> lands in Mailpit.'),
        ('Can I keep my .local addresses?', 'Lumina uses <code>.test</code> by default, which is reserved for local use. You can change the suffix on the Sites page.'),
    ],
})

# The overview table shows these rows only.
OVERVIEW_ROWS = ['price', 'source', 'windows', 'macos', 'linux', 'sites', 'https', 'node', 'db']


def cell(value):
    esc = html.escape
    if not value:
        return '<td class="unk"><span title="Not confirmed">—</span></td>'
    marks = {'+': ('yes', '✓', 'Yes'), '-': ('no', '✕', 'No'), '~': ('part', '◐', 'Partly')}
    if value[0] in marks:
        kind, glyph, label = marks[value[0]]
        return (f'<td><span class="mk {kind}" aria-hidden="true">{glyph}</span>'
                f'<span class="sr">{label}: </span>{esc(value[1:])}</td>')
    return f'<td>{esc(value)}</td>'


def compare_table(tools, rows):
    head = ''.join(f'<th scope="col">{html.escape(t["name"])}</th>' for t in tools)
    body = '\n'.join(
        f'          <tr><th scope="row">{label}</th>'
        + ''.join(cell((t if t is LUMINA else t['cells']).get(key, '')) for t in tools) + '</tr>'
        for key, label in ROWS if key in rows)
    return f'''      <div class="cmp-wrap">
        <table class="cmp">
          <thead><tr><th scope="col"><span class="sr">Feature</span></th>{head}</tr></thead>
          <tbody>
{body}
          </tbody>
        </table>
      </div>'''


def subpage(path, title, description, ld, crumbs, body, current=''):
    """An English page below the site root, with the shared head, header and footer."""
    url = BASE + path
    up = '../' * path.count('/')
    esc = html.escape
    og_image = BASE + 'assets/og-en.png'
    links = [(up + '#features', 'Features'), (up + '#download', 'Download'),
             (up + 'compare/', 'Compare'), (up + 'guides/', 'Guides'), (up + 'changelog/', 'Changelog')]
    nav = '\n'.join(f'        <a href="{h}"{" aria-current=\"page\"" if n == current else ""}>{n}</a>'
                    for h, n in links)
    trail = '<span aria-hidden="true">/</span>'.join(
        f'<a href="{up + p}">{esc(n)}</a>' if p != path else f'<span aria-current="page">{esc(n)}</span>'
        for n, p in crumbs)
    return f'''<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{esc(title)}</title>
  <meta name="description" content="{esc(description)}">
  <meta name="robots" content="index, follow, max-image-preview:large, max-snippet:-1">
  <meta name="author" content="{AUTHOR}">
  <meta name="application-name" content="Lumina">
  <link rel="canonical" href="{url}">
  <meta name="theme-color" content="#1b1e20" media="(prefers-color-scheme: dark)">
  <meta name="theme-color" content="#ffffff" media="(prefers-color-scheme: light)">
  <meta name="color-scheme" content="dark light">
  <link rel="icon" href="{up}assets/icon.svg" type="image/svg+xml">
  <link rel="icon" href="{up}assets/icon-32.png" sizes="32x32" type="image/png">
  <link rel="apple-touch-icon" href="{up}assets/icon-180.png">
  <link rel="manifest" href="{up}site.webmanifest">
  <meta property="og:type" content="website">
  <meta property="og:site_name" content="Lumina">
  <meta property="og:title" content="{esc(title)}">
  <meta property="og:description" content="{esc(description)}">
  <meta property="og:url" content="{url}">
  <meta property="og:image" content="{og_image}">
  <meta property="og:image:width" content="1200">
  <meta property="og:image:height" content="630">
  <meta property="og:locale" content="en_US">
  <meta name="twitter:card" content="summary_large_image">
  <meta name="twitter:title" content="{esc(title)}">
  <meta name="twitter:description" content="{esc(description)}">
  <meta name="twitter:image" content="{og_image}">
  <link rel="preload" href="{up}assets/fonts/manrope-latin.woff2" as="font" type="font/woff2" crossorigin>
  <link rel="stylesheet" href="{up}assets/style.css">
  <script type="application/ld+json">{json.dumps(ld, ensure_ascii=False).replace('</', '<\\/')}</script>
</head>
<body class="lang-en">
  <a class="skip" href="#main">Skip to content</a>
  <header class="top">
    <nav class="wrap nav" aria-label="Main">
      <a class="brand" href="{up}"><img src="{up}assets/icon.svg" alt="" width="30" height="30"><span>Lumina</span></a>
      <div class="nav-links">
{nav}
        <a class="icon-link" href="{REPO}" aria-label="GitHub">{icon('github', 20)}</a>
      </div>
    </nav>
  </header>

  <main id="main" class="wrap compare">
    <nav class="crumbs" aria-label="Breadcrumb">{trail}</nav>
{body}
  </main>

  <footer class="foot">
    <div class="wrap foot-inner">
      <div>
        <a class="brand" href="{up}"><img src="{up}assets/icon.svg" alt="" width="26" height="26"><span>Lumina</span></a>
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


def breadcrumb_ld(url, crumbs):
    return {'@type': 'BreadcrumbList', '@id': url + '#breadcrumb',
            'itemListElement': [{'@type': 'ListItem', 'position': i, 'name': n, 'item': BASE + p}
                                for i, (n, p) in enumerate(crumbs, 1)]}


def cta_html(up):
    return f'''      <div class="cta">
        <a class="btn primary" href="{up}#download">{icon('download', 20)}<span>Download Lumina, free</span></a>
        <a class="btn" href="{REPO}">{icon('github', 20)}<span>View on GitHub</span></a>
      </div>'''


def compare_page(c):
    """site/compare/<tool>/index.html: Lumina next to one tool."""
    path = f'compare/{c["slug"]}/'
    url = BASE + path
    esc = html.escape
    crumbs = [('Lumina', ''), ('Compare', 'compare/'), (f'Lumina vs {c["name"]}', path)]
    ld = {'@context': 'https://schema.org', '@graph': [
        {'@type': ['WebPage', 'FAQPage'], '@id': url + '#webpage', 'url': url, 'name': c['title'],
         'description': c['description'], 'inLanguage': 'en', 'isPartOf': {'@id': BASE + '#website'},
         'about': {'@id': BASE + '#software'}, 'breadcrumb': {'@id': url + '#breadcrumb'},
         'mainEntity': [{'@type': 'Question', 'name': q,
                         'acceptedAnswer': {'@type': 'Answer', 'text': re.sub('<[^>]+>', '', a)}}
                        for q, a in c['faq']]},
        breadcrumb_ld(url, crumbs),
    ]}
    why = '\n'.join(f'''        <li class="card feature">
          <h3>{esc(h)}</h3>
          <p>{p}</p>
        </li>''' for h, p in c['why'])
    move = '\n'.join(f'        <li>{s}</li>' for s in c['move'])
    faq = '\n'.join(f'''      <details class="faq-item">
        <summary>{esc(q)}</summary>
        <p>{a}</p>
      </details>''' for q, a in c['faq'])
    others = ' · '.join(f'<a href="../{o["slug"]}/">Lumina vs {esc(o["name"])}</a>'
                        for o in COMPETITORS if o is not c)
    body = f'''    <header class="cl-head">
      <h1>Lumina vs {esc(c['name'])}</h1>
      <p class="lead">{c['intro']}</p>
{cta_html('../../')}
    </header>

    <section class="cmp-section" aria-labelledby="table-title">
      <h2 id="table-title">Side by side</h2>
{compare_table([LUMINA, c], [k for k, _ in ROWS])}
      <p class="cmp-note">✓ yes · ✕ no · ◐ only in a paid edition or partly · — not confirmed. {esc(c['name'])} details from <a href="{c['site']}" rel="nofollow noopener">its website</a>, checked in {CHECKED}. Spotted a mistake? <a href="{REPO}/issues/new/choose">Tell us</a>.</p>
    </section>

    <section class="cmp-section" aria-labelledby="why-title">
      <h2 id="why-title">Why developers switch from {esc(c['name'])}</h2>
      <ul class="grid features">
{why}
      </ul>
    </section>

    <section class="cmp-section narrow" aria-labelledby="move-title">
      <h2 id="move-title">Move your projects from {esc(c['name'])}</h2>
      <ol class="move-steps">
{move}
      </ol>
    </section>

    <section class="cmp-section narrow" aria-labelledby="faq-title">
      <h2 id="faq-title">Questions</h2>
{faq}
    </section>

    <section class="cmp-section cmp-end" aria-labelledby="try-title">
      <h2 id="try-title">Try Lumina</h2>
      <p class="section-lead">Free and open source, for Windows, macOS and Linux.</p>
{cta_html('../../')}
      <p class="cmp-others">More comparisons: {others} · <a href="../">all side by side</a></p>
    </section>'''
    return path, subpage(path, c['title'], c['description'], ld, crumbs, body, 'Compare')


def compare_hub():
    """site/compare/index.html: every tool side by side, with links to the pages."""
    path = 'compare/'
    url = BASE + path
    esc = html.escape
    title = 'Lumina vs Laragon, XAMPP, Herd, MAMP and Local: local dev tools compared'
    description = ('Compare local development environments side by side: Lumina, Laragon, XAMPP, Laravel Herd, '
                   'MAMP and Local. Price, open source, Windows, macOS and Linux, .test sites, HTTPS and databases.')
    crumbs = [('Lumina', ''), ('Compare', path)]
    ld = {'@context': 'https://schema.org', '@graph': [
        {'@type': 'CollectionPage', '@id': url + '#webpage', 'url': url, 'name': title,
         'description': description, 'inLanguage': 'en', 'isPartOf': {'@id': BASE + '#website'},
         'about': {'@id': BASE + '#software'}, 'breadcrumb': {'@id': url + '#breadcrumb'},
         'hasPart': [{'@type': 'WebPage', 'url': BASE + f'compare/{c["slug"]}/', 'name': c['title']}
                     for c in COMPETITORS]},
        breadcrumb_ld(url, crumbs),
    ]}
    cards = '\n'.join(f'''        <li class="card feature">
          <h3><a href="{c['slug']}/">Lumina vs {esc(c['name'])}</a></h3>
          <p>{esc(c['description'])}</p>
        </li>''' for c in COMPETITORS)
    body = f'''    <header class="cl-head">
      <h1>Local development tools compared</h1>
      <p class="lead">Lumina, Laragon, XAMPP, Laravel Herd, MAMP and Local all run PHP, a web server and a database on your own computer. They differ in price, in the systems they run on and in what is included. Here they are side by side.</p>
{cta_html('../')}
    </header>

    <section class="cmp-section" aria-labelledby="table-title">
      <h2 id="table-title">At a glance</h2>
{compare_table([LUMINA] + COMPETITORS, OVERVIEW_ROWS)}
      <p class="cmp-note">✓ yes · ✕ no · ◐ only in a paid edition or partly · — not confirmed. Details from each product's website, checked in {CHECKED}. Spotted a mistake? <a href="{REPO}/issues/new/choose">Tell us</a>.</p>
    </section>

    <section class="cmp-section" aria-labelledby="pages-title">
      <h2 id="pages-title">In detail</h2>
      <ul class="grid features">
{cards}
      </ul>
    </section>'''
    return path, subpage(path, title, description, ld, crumbs, body, 'Compare')


def article_page(a, path, crumbs, kind):
    """A guide (kind 'guide') or a stack page ('stack') from site_content.py."""
    url = BASE + path
    esc = html.escape
    up = '../' * path.count('/')
    ld = {'@context': 'https://schema.org', '@graph': [
        {'@type': ['TechArticle' if kind == 'guide' else 'WebPage', 'FAQPage'], '@id': url + '#webpage',
         'url': url, 'name': a['title'], 'headline': a['h1'], 'description': a['description'],
         'inLanguage': 'en', 'isPartOf': {'@id': BASE + '#website'}, 'about': {'@id': BASE + '#software'},
         'author': {'@type': 'Person', 'name': AUTHOR, 'url': AUTHOR_URL},
         'breadcrumb': {'@id': url + '#breadcrumb'},
         'mainEntity': [{'@type': 'Question', 'name': q,
                         'acceptedAnswer': {'@type': 'Answer', 'text': re.sub('<[^>]+>', '', ans)}}
                        for q, ans in a['faq']]},
        breadcrumb_ld(url, crumbs),
    ]}
    sections = '\n'.join(f'''    <section class="cmp-section narrow article">
      <h2>{esc(h)}</h2>
      {body}
    </section>''' for h, body in a['sections'])
    faq = '\n'.join(f'''      <details class="faq-item">
        <summary>{esc(q)}</summary>
        <p>{ans}</p>
      </details>''' for q, ans in a['faq'])
    guides = {g['slug']: g for g in GUIDES}
    related = '\n'.join(f'''        <li class="card feature">
          <h3><a href="{up}guides/{s}/">{esc(guides[s]['h1'])}</a></h3>
          <p>{esc(guides[s]['description'])}</p>
        </li>''' for s in a['related'])
    body = f'''    <header class="cl-head">
      <h1>{esc(a['h1'])}</h1>
      <p class="lead">{a['intro']}</p>
{cta_html(up)}
    </header>
{sections}

    <section class="cmp-section narrow" aria-labelledby="faq-title">
      <h2 id="faq-title">Questions</h2>
{faq}
    </section>

    <section class="cmp-section" aria-labelledby="more-title">
      <h2 id="more-title">{'Related guides' if kind == 'guide' else 'Guides'}</h2>
      <ul class="grid features">
{related}
      </ul>
    </section>'''
    return path, subpage(path, a['title'], a['description'], ld, crumbs, body, 'Guides' if kind == 'guide' else '')


def guide_pages():
    pages = [article_page(g, f'guides/{g["slug"]}/', [('Lumina', ''), ('Guides', 'guides/'),
                                                       (g['h1'], f'guides/{g["slug"]}/')], 'guide')
             for g in GUIDES]
    pages += [article_page(f, f'{f["slug"]}/', [('Lumina', ''), (f['h1'], f'{f["slug"]}/')], 'stack')
              for f in FRAMEWORKS]
    return pages


def guides_hub():
    path = 'guides/'
    url = BASE + path
    esc = html.escape
    title = 'Local development guides: PHP versions, .test domains, HTTPS, Laravel, Next.js'
    description = ('Step-by-step guides for local web development with Lumina: several PHP versions, .test '
                   'domains, trusted HTTPS, Laravel, WordPress, Next.js and Django, on Windows, macOS and Linux.')
    crumbs = [('Lumina', ''), ('Guides', path)]
    ld = {'@context': 'https://schema.org', '@graph': [
        {'@type': 'CollectionPage', '@id': url + '#webpage', 'url': url, 'name': title,
         'description': description, 'inLanguage': 'en', 'isPartOf': {'@id': BASE + '#website'},
         'breadcrumb': {'@id': url + '#breadcrumb'},
         'hasPart': [{'@type': 'TechArticle', 'url': BASE + f'guides/{g["slug"]}/', 'headline': g['h1']}
                     for g in GUIDES]},
        breadcrumb_ld(url, crumbs),
    ]}
    cards = lambda items, prefix: '\n'.join(f'''        <li class="card feature">
          <h3><a href="{prefix}{i['slug']}/">{esc(i['h1'])}</a></h3>
          <p>{esc(i['description'])}</p>
        </li>''' for i in items)
    body = f'''    <header class="cl-head">
      <h1>Guides</h1>
      <p class="lead">Short, practical guides for running web projects on your own computer with Lumina.</p>
    </header>

    <section class="cmp-section" aria-labelledby="how-title">
      <h2 id="how-title">How to</h2>
      <ul class="grid features">
{cards(GUIDES, '')}
      </ul>
    </section>

    <section class="cmp-section" aria-labelledby="stack-title">
      <h2 id="stack-title">By stack</h2>
      <ul class="grid features">
{cards(FRAMEWORKS, '../')}
      </ul>
    </section>'''
    return path, subpage(path, title, description, ld, crumbs, body, 'Guides')


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
    </image:image>''' + ''.join(f'''
    <image:image>
      <image:loc>{BASE}assets/screens/{n}@2x.webp</image:loc>
    </image:image>''' for n, _, _ in t['shots']) + '''
  </url>''' for t in T.values())
    # The changelog and the comparisons are in English only: no language versions.
    for path in ['changelog/'] + [p for p, _ in COMPARE_PAGES]:  # comparisons, guides and stacks
        urls += f'''
  <url>
    <loc>{BASE}{path}</loc>
    <lastmod>{lastmods[BASE + path]}</lastmod>
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
COMPARE_PAGES = [compare_hub()] + [compare_page(c) for c in COMPETITORS] + [guides_hub()] + guide_pages()
for path, text in COMPARE_PAGES:
    os.makedirs(ROOT + path, exist_ok=True)
    pages.append((ROOT + path + 'index.html', BASE + path, text))
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
