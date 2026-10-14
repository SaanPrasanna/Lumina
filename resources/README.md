# Bundled resources

Everything here is copied into the installed app (`bundle.resources` in
`tauri.conf.json`) and is **read-only at runtime**. On first run Lumina renders these
templates into the user's runtime directory (`etc/`), never in place.

- `config/default.toml` — initial `config.toml`
- `templates/` — Tera templates for server configs (`*.tera`)
- `catalog/` — offline fallback of the Quick Add package catalog
- `quickapp/` — QuickApp recipes
- `www/` — default landing page served at `localhost`
