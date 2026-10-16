<?xml version="1.0" encoding="UTF-8"?>
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
