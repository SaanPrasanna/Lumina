# Bundled resources

Everything here is copied into the installed app (`bundle.resources` in
`tauri.conf.json`) and is **read-only at runtime**. On first run Lumina renders these
templates into the user's runtime directory (`etc/`), never in place.

- `config/default.toml` — initial `config.toml`
- `templates/` — Tera templates for server configs (`*.tera`)
- `catalog/` — offline fallback of the Quick Add package catalog
- `quickapp/` — QuickApp recipes
- `www/` — default landing page served at `localhost`

## Third-party files
- `templates/nginx/mime.types` and `templates/nginx/fastcgi_params` are copied
  unchanged from the nginx 1.30.5 distribution (BSD-2-Clause, see
  `templates/nginx/LICENSE.nginx`). Bundling them keeps the generated config the
  same on every OS, regardless of where a distribution installs nginx.
