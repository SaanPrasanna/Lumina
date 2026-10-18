# Lumina roadmap: the AI era and full-stack web development

Lumina today is a strong local PHP stack: supervised services, automatic
`.test` sites with trusted HTTPS, Quick Add, QuickApp, profiles, PATH shims,
ngrok sharing and Mailpit. This document proposes where to take it next, for
two shifts in how web software is built:

1. **AI coding agents** (Claude Code, Cursor, Copilot, Codex and others) now
   write, run and debug much of the code. They work best when they can see and
   control the environment: which services run, where the logs are, what the
   database holds, which URL a site has.
2. **Web development is polyglot.** A typical project mixes a PHP or Python
   backend, a Node.js frontend (Vite, Next.js, Nuxt, Astro), a database, a
   cache, search, queues, object storage and, more and more, an LLM.

The goal: **Lumina becomes the local platform that both the developer and
their AI agent run the project on**, for any web stack, without Docker or a
VM, and without sending anything off the machine unless asked.

Ideas are grouped by theme, then ranked in [Priorities](#priorities). Each one
notes where it fits in the current code.

---

## Guiding principles

These follow from what Lumina already promises and should hold for every
feature below.

- **Local first, private by default.** No AI feature sends code, logs or data
  to a cloud service unless the user turns it on and picks the provider.
  Local models are always an option. No telemetry.
- **Bring your own model.** Support local models (Ollama, llama.cpp) and the
  user's own API keys (Anthropic, OpenAI, Google, any OpenAI-compatible
  endpoint), kept in the OS keychain like the ngrok token.
- **Agents get the same guarantees as people.** Everything an agent can do goes
  through the orchestrator, so ordering, port checks and "no ghost processes"
  still apply. Destructive actions need the user's approval.
- **One manager, many runtimes.** New languages and services reuse the
  supervisor, the catalog (SHA-256 verified downloads), the shims and the
  vhost generator rather than adding parallel systems.

---

## 1. Make Lumina usable by AI agents

### 1.1 A Lumina MCP server *(highest impact)*

The [Model Context Protocol](https://modelcontextprotocol.io/) is the common
way coding agents call local tools. A `lumina-mcp` sidecar (stdio, plus a
local HTTP transport) would let any MCP-capable agent:

| Tool | What the agent gets |
|---|---|
| `services_status`, `service_start`, `service_stop`, `service_restart` | Run the stack it needs, see what is up and on which port |
| `logs_tail`, `logs_search` | Read PHP, nginx, database and app logs, the first thing an agent needs when a page fails |
| `sites_list`, `site_info` | The URL, document root, PHP version and HTTPS state of each project |
| `db_list`, `db_schema`, `db_query` | Inspect tables and run **read-only** queries; writes need approval |
| `mail_list`, `mail_read` | Read Mailpit messages to check sign-up, reset-password and notification mail |
| `php_set_version`, `package_install` | Switch or install a runtime when the project requires it (with approval) |
| `tunnel_open` | Share a site for a webhook or a phone test (with approval) |

Fit: a new crate next to `lumina-tunnel`, talking to the running app over a
local socket the way the second-instance CLI flags already do (`cli.rs`).
Every call goes through `Orchestrator`. A Settings page lists connected agents,
which tools each may use, and a log of what they did. "Connect to Claude Code
/ Cursor / VS Code" buttons write the client config for the user.

### 1.2 A `lumina` command-line interface

Agents and scripts work best in a terminal. Extend today's flags (`--stop-all`,
`--start-profile`) into a full CLI on the PATH shim folder:

```
lumina status                 lumina logs php --follow
lumina start mysql redis      lumina site info blog
lumina db query blog "select count(*) from users"
lumina mail last --to user@example.com
```

Machine-readable output (`--json`) makes it as useful to agents as MCP, for
tools that only run shell commands. The MCP server and the CLI should share one
implementation.

### 1.3 Project context files for agents

When QuickApp creates a project, or on request for an existing one, write an
`AGENTS.md` (read by most agents; a `CLAUDE.md` that points to it for Claude
Code) with facts the agent would otherwise guess wrong:

- the URL (`https://blog.test`), the document root and the PHP/Node versions;
- database host, port, name and user; Redis, Mailpit and search endpoints;
- the commands that work here (`php artisan …`, `npm run dev`, tests);
- "use `lumina logs php` to see errors; mail goes to Mailpit".

Cheap to build (a template in `resources/quickapp`) and it immediately makes
agents more accurate. Keep it regenerated when versions or ports change, in a
marked block so the user's own notes survive.

### 1.4 A project manifest: `lumina.toml`

A small file committed with the project that declares what it needs:

```toml
[runtime]
php = "8.4"
node = "22"

[services]
database = "mysql 8.4"
redis = true
mailpit = true
search = "meilisearch"

[processes]
vite = "npm run dev"
queue = "php artisan queue:work"
scheduler = "php artisan schedule:work"
```

Opening the folder offers to install what is missing and start it all. It is
the per-project form of today's profiles, works for teammates who clone the
repo, and gives agents one place to read and edit the environment. It can also
be generated from an existing `docker-compose.yml`, `.nvmrc`, `composer.json`
or `.tool-versions` (the Docker Compose import idea in the post-1.0 backlog).

---

## 2. AI inside the developer's workflow

### 2.1 Local LLMs as a service

Add **Ollama** (or the llama.cpp server) to Quick Add as a supervised service,
like Redis: start/stop, a model manager (pull, remove, disk use), and an
OpenAI-compatible endpoint at a stable address such as `https://ai.test/v1`.
Apps under development can call an LLM with no API key and no cost, and work
offline. Show whether a GPU is used and the RAM each model needs.

### 2.2 An "AI Mailpit": inspect and record LLM calls

Mailpit catches outgoing mail; do the same for outgoing LLM requests. A local
proxy that apps point their `*_BASE_URL` at, which forwards to the real
provider (or to the local model) and records each call:

- the prompt, response, model, tokens, latency and **estimated cost**;
- streaming shown live; tool calls and structured output pretty-printed;
- **record and replay**: answer repeated requests from a cache, so tests and
  UI work run offline, fast and free, and give the same output every time;
- a mock mode that returns fixed responses or errors (rate limits, timeouts)
  to test how the app copes.

Nothing like this exists as a one-click local tool for PHP and Node developers,
and it fits the Mailpit pattern exactly (a supervised service plus an inbox UI).

### 2.3 Vector databases and search

AI features (RAG, semantic search, recommendations) need somewhere to keep
embeddings. Add to the catalog:

- **pgvector** for the PostgreSQL Lumina already runs: one switch per
  database, the most natural first step;
- **Qdrant** or **Chroma** as standalone vector databases;
- **Meilisearch** and **Typesense** for search (Laravel Scout, InstantSearch),
  which now include hybrid and vector search too.

### 2.4 "Explain this error"

On the Logs page and in failed-start messages, one click sends the log lines,
the service's config and the versions involved to the model the user chose
(local by default) and shows a likely cause and fix. Paths, passwords and keys
are redacted before anything leaves the app, and the user sees exactly what is
sent. The same helper can explain port conflicts, failed QuickApp steps and
SSL trust problems.

### 2.5 Describe a project, get a project

QuickApp today asks for a framework and options. An optional prompt box
("Laravel API with Vue frontend, PostgreSQL, queues and search") can choose the
recipe, options and services, show the plan for the user to confirm, then run
the normal recipe. The model only fills in the recipe's choices; it never runs
arbitrary commands.

### 2.6 Secrets for AI keys

Projects now hold several API keys (LLM providers, Stripe, mail). Keep them in
the OS keychain (reusing `lumina-tunnel::token`), inject them into a project's
`.env` on request, and warn when a real key is about to be committed or is
present in a shared tunnel. Agents can see that a key exists but not its value.

---

## 3. Full-stack web development, beyond PHP

> **Status: built** (Unreleased, see CHANGELOG.md): 3.1–3.7 all landed, apart
> from Go and Ruby (3.2) and RabbitMQ (3.4), which stay ideas until there is
> demand.

### 3.1 Run any dev server behind `<name>.test`

The biggest step from "PHP stack" to "web stack". When a project is a Node.js
(Vite, Next.js, Nuxt, SvelteKit, Astro, Remix), Python (Django, FastAPI,
Flask) or other app, Lumina:

1. runs its dev command under the supervisor (with the right Node/Python from
   the shims), picking a free port;
2. points `https://<name>.test` at it through nginx `proxy_pass`, with
   WebSocket upgrade for hot reload, using the same certificates;
3. shows its output on the Logs page and restarts it on crash.

Fit: a new site type in `lumina-vhost` (proxy instead of FastCGI) and the
"Node/Python project runners" item in the post-1.0 backlog. With `lumina.toml`
`[processes]`, a Laravel app can run `php-fpm` + `vite` + a queue worker
together, which is what most Laravel projects need anyway.

### 3.2 More runtimes and package managers

- **Bun** and **Deno** next to Node.js; **pnpm** and **Yarn** through Corepack.
- **uv** for Python (fast installs, per-project virtual environments).
- **Go** and **Ruby** later, if demand shows.
- Per-project versions read from `.nvmrc`, `.node-version`,
  `.python-version`, `composer.json` (`require.php`) and `.tool-versions`.

### 3.3 Per-site PHP version

Already in the post-1.0 backlog and still one of the most requested features
in tools like this: one php-cgi pool per PHP version in use, each site's vhost
pointing at its pool. It removes the main reason people switch profiles.

### 3.4 More services in Quick Add

| Need | Candidates |
|---|---|
| Document database | MongoDB |
| Cache (Redis-compatible, BSD licensed) | Valkey |
| Object storage (S3 API) | An S3-compatible server, so uploads work as in production |
| Queues | RabbitMQ; Redis/Valkey streams already cover many apps |
| Search | Meilisearch, Typesense (see 2.3) |
| Database UI for every engine | Adminer next to phpMyAdmin, so PostgreSQL and SQLite get a web UI |

Each is a catalog entry plus a small service module like `mailpit.rs`.

### 3.5 Debugging and inspection

- **Xdebug toggle** (in the backlog): one switch per PHP version, with
  ready-made VS Code / PhpStorm launch settings written into the project.
- **Request inspector**: a recent-requests view per site (method, path,
  status, time, slow queries), fed from the web server's logs. Agents can read
  it through MCP.
- **Webhook catcher**: a local endpoint that records incoming webhooks and
  replays them to a site, together with tunnels for providers that must reach
  the machine.

### 3.6 Testing tools

- Install **Playwright** browsers from Quick Add and run end-to-end tests
  against `https://<name>.test` (the local CA is already trusted).
- Database **snapshots**: save and restore a project's database in one click,
  before a migration or between test runs. Useful for agents too: snapshot,
  let the agent experiment, roll back.

### 3.7 Sharing and previews

- **Cloudflare Tunnel** as a second provider behind the existing
  `TunnelProvider` trait (in the backlog): stable addresses, no account limits
  on tunnels.
- **LAN sharing** (servers currently listen on `127.0.0.1` only): test on a
  phone on the same Wi-Fi without any tunnel.

---

## Priorities

Ranked by value to users against effort, given the current architecture.

| # | Feature | Why now | Effort |
|---|---|---|---|
| 1 | `AGENTS.md` context files (1.3) | Immediate agent accuracy gain; template work only | S |
| 2 | `lumina` CLI with `--json` (1.2) | Foundation for agents, scripts and MCP | M |
| 3 | MCP server (1.1) | Turns Lumina into the environment agents run on | M |
| 4 | Dev servers behind `<name>.test` (3.1) | Opens Lumina to Node/Python frontends and full-stack apps | L |
| 5 | Local LLM service (2.1) + pgvector (2.3) | Every new app wants an LLM and embeddings | M |
| 6 | LLM call inspector with replay (2.2) | Distinctive; nothing similar is one click away | M |
| 7 | `lumina.toml` project manifest (1.4) | Reproducible per-project stacks for teams and agents | M |
| 8 | Per-site PHP version (3.3) | Long-standing request; removes profile switching | M |
| 9 | Database snapshots (3.6) | Safety net for migrations and agent experiments | S |
| 10 | "Explain this error" (2.4) | Helpful once providers and redaction exist (from 2.1/2.6) | S |
| 11 | More services, Xdebug, Cloudflare Tunnel, LAN sharing | Steady catalog growth on proven patterns | S each |

S = days, M = one to three weeks, L = a month or more.

A sensible sequence: **1 → 2 → 3** as one "agent-ready" release (they share
code and the story is clear), then **4** as the "beyond PHP" release, then the
AI services (**5, 6**), then the rest.

---

## What to avoid

- **Bundling a chat assistant into the app.** Developers already have agents
  in their editor and terminal; Lumina adds more value by making those agents
  effective (MCP, CLI, context) than by competing with them.
- **AI features that require a cloud account.** Every AI feature should work
  with a local model or be clearly optional.
- **Letting agents bypass safeguards.** No raw shell access through MCP, no
  writes to databases or files outside a project without approval, and every
  action logged.
- **Becoming Docker.** Keep the native, fast, no-VM approach; read
  `docker-compose.yml` to set up native services, don't run containers.

---

## Open questions

- Should the MCP server run only while the app runs, or also start the stack
  on demand when an agent connects?
- Which local model runtime to standardise on (Ollama is the most familiar;
  llama.cpp is smaller and easier to bundle)?
- How much of `lumina.toml` should be compatible with existing formats
  (`.tool-versions`, devcontainer features) rather than new?
- Pricing data for the LLM inspector changes often: ship it in the catalog and
  update it with Lumina's releases, or let users edit it?
