# Self-hosting Campbooks

Campbooks is source-available under the Sustainable Use License — you're free to self-host it **for free,
for your own business or personal use**. You can run your own instance with Docker in a
few minutes. This guide covers the full setup: the stack, the secrets you need,
and how to wire up each external integration.

> Looking to just try it locally? Jump to [Quick start](#quick-start). Running it
> on a server for real? Also read [Production](#running-in-production).

---

## What you get

`docker compose up` runs three containers:

| Service    | What it is                                              |
|------------|---------------------------------------------------------|
| `postgres` | PostgreSQL **with the pgvector extension** (required)   |
| `web`      | The Rails app (Puma behind Thruster)                    |
| `worker`   | Background jobs — email scanning, AI, indexing (Solid Queue) |

Postgres holds everything: app data, cache, jobs, and Action Cable (via the
Solid* adapters), plus vector embeddings for semantic search (pgvector). A fourth
service, `opensearch`, is **optional** and only powers contact full-text
autocomplete — see [Search](#search-opensearch).

Uploaded files (email attachments saved as Documents) are stored on a local
Docker volume by default, or in S3-compatible object storage if you configure it.

---

## Prerequisites

- **Docker** and the **Docker Compose** plugin (Docker Desktop, or Docker Engine
  ≥ 24 with `docker compose`).
- About **2 GB RAM** free for the default stack (add ~1 GB if you enable OpenSearch).
- `openssl` on your machine (for generating secrets — already present on macOS/Linux).
- Optional, for a real deployment: a domain name and a reverse proxy that
  terminates TLS (e.g. Caddy, Traefik, nginx).

---

## Quick start

```bash
git clone https://github.com/notacamp/campbooks.git
cd campbooks

cp .env.example .env
bin/generate-secrets            # fills SECRET_KEY_BASE, the AR encryption keys, and the DB password

# (optional) edit .env to add an AI key and any integrations you want
docker compose up -d --build

# watch it come up; first boot creates the databases and runs migrations
docker compose logs -f web
```

Then open <http://localhost:3000> and **register the first account** — on a
self-hosted instance signup is open, and the first user creates the workspace.

> First boot also seeds a demo workspace with a login `admin@example.com` /
> `changeme123` (and `partner@example.com`). **Change that password after first
> login**, or set `SEED_PASSWORD` in `.env` before the first boot. If you don't
> want the demo data, just register your own account and delete the demo workspace.

That's it. The app runs fully without any external API keys; AI features and
mailbox connections light up as you add credentials (below).

To stop: `docker compose down` (add `-v` to also delete the database/uploads).

---

## Configuration

All configuration is environment variables in `.env`. `bin/generate-secrets`
handles the required secrets; everything else is optional. The annotated list
lives in [`.env.example`](../.env.example). The essentials:

### Required (generated for you)

| Variable | Notes |
|---|---|
| `SECRET_KEY_BASE` | Rails session/signing key. `openssl rand -hex 64` |
| `ACTIVE_RECORD_PRIMARY_KEY` | Encrypts OAuth tokens & AI keys at rest. `openssl rand -hex 16` |
| `ACTIVE_RECORD_DETERMINISTIC_KEY` | As above |
| `ACTIVE_RECORD_KEY_DERIVATION_SALT` | As above |
| `CAMPBOOKS_DATABASE_PASSWORD` | Password for the bundled Postgres |

> You do **not** need `config/master.key` — self-hosting reads `SECRET_KEY_BASE`
> from the environment.

### Required (you set)

| Variable | Default | Notes |
|---|---|---|
| `APP_HOST` | `localhost` | The hostname you reach the app on. Must match the `Host` header (localhost, an IP, or your domain) — other hosts are rejected for security. |
| `FORCE_SSL` | `false` | Keep `false` for plain-HTTP local use. Set `true` only behind a TLS proxy (see [Production](#running-in-production)). |
| `SELF_HOSTED` | `1` | Open registration + reads AI keys from env. |

---

## External integrations

Every integration is optional. Add the ones you want; skip the rest.

For each OAuth provider you must register an app in that provider's console and
whitelist the **callback URL**, which is `<your-app-url>/oauth/<provider>/callback`
— e.g. `http://localhost:3000/oauth/gmail/callback` locally, or
`https://app.example.com/oauth/gmail/callback` in production.

### AI providers

AI is what makes Campbooks Campbooks (triage, the Scout assistant, draft replies,
document analysis, semantic search) — but it's all optional and off until you add
a key. You can set keys here for the whole instance, or let each user enter their
own in **Settings → AI** (stored encrypted per workspace).

| Variable | Unlocks |
|---|---|
| `OPENAI_API_KEY` | Text AI **+ document/vision analysis + embeddings** (semantic search). The most capable single key. |
| `ANTHROPIC_API_KEY` | Text AI (Claude) |
| `MISTRAL_API_KEY` | Text AI (EU-hosted) |
| `DEEPSEEK_API_KEY` | Text AI |
| `GEMINI_API_KEY` | Text AI + embeddings |

- For the assistant/triage: set **any one** text provider.
- For attachment/document analysis and the best search: set **`OPENAI_API_KEY`**.
- With no embeddings provider (OpenAI or Gemini), search falls back to keyword matching.

### Google — Gmail & Calendar

Lets users sign in with Google and connect a Gmail mailbox (which also syncs
their Google Calendar on the same grant).

1. [Google Cloud Console](https://console.cloud.google.com) → create a project →
   **APIs & Services → Credentials → OAuth client ID** (type: Web application).
2. Enable the **Gmail API** and **Google Calendar API** for the project.
3. Add the redirect URI: `<your-app-url>/oauth/gmail/callback`.
4. Set `GOOGLE_CLIENT_ID` and `GOOGLE_CLIENT_SECRET`.

### Google Drive — "Send to Drive"

The interactive Drive export uses the **full `drive` scope**, which is a Google
*restricted* scope, so it needs its **own, separately verified** OAuth app.

1. Create a second OAuth client (or a separate project).
2. Add the scope `https://www.googleapis.com/auth/drive` and the redirect URI
   `<your-app-url>/oauth/google/callback`.
3. Set `GOOGLE_DRIVE_CLIENT_ID` and `GOOGLE_DRIVE_CLIENT_SECRET`.

### Zoho Mail

1. [Zoho API Console](https://api-console.zoho.com) → **Server-based Application**.
2. Redirect URI: `<your-app-url>/oauth/zoho/callback`.
3. Set `ZOHO_CLIENT_ID`, `ZOHO_CLIENT_SECRET`, and `ZOHO_REGION` (`eu`, `com`,
   `in`, `au`, `jp` — match your Zoho data center).

### Microsoft 365 / Outlook

> **Not production-ready — disabled by default.** Every Microsoft surface (the
> "Sign in with Microsoft" button, "Connect Microsoft 365", and the OAuth
> callbacks) is hidden unless `ENABLE_MICROSOFT=1`. Leave it unset to keep
> Microsoft off entirely.

1. [Entra admin center](https://entra.microsoft.com) → **App registrations** →
   New registration. Supported account types: *Accounts in any organizational
   directory* (work/school accounts; the app uses the `/organizations/` endpoint).
2. Add a **Web** redirect URI: `<your-app-url>/oauth/microsoft/callback`.
3. Create a **client secret**.
4. Set `MICROSOFT_CLIENT_ID`, `MICROSOFT_CLIENT_SECRET`, and `ENABLE_MICROSOFT=1`
   (the last reveals the Microsoft sign-in + "Connect Microsoft 365" surfaces).

### Notion — "Send to Notion"

1. [notion.so/my-integrations](https://www.notion.so/my-integrations) → new
   integration, type **Public**, with Read/Insert content capabilities.
2. Redirect URI: `<your-app-url>/oauth/notion/callback`.
3. Set `NOTION_CLIENT_ID` and `NOTION_CLIENT_SECRET`.

If you leave these unset, users can still connect Notion by pasting an internal
integration token in Settings.

### Outbound email (SMTP)

Without SMTP, the app sends no email (signup OTP codes, notifications, reports
are skipped — fine for a single-user trial, but you'll want it for real use).

```bash
SMTP_ADDRESS=smtp.example.com
SMTP_PORT=587
SMTP_USERNAME=you@example.com
SMTP_PASSWORD=your_smtp_password
MAILER_FROM=Campbooks <no-reply@example.com>
```

### File storage (S3)

By default, uploads live on the `storage` Docker volume. To use object storage
(recommended if you run multiple web replicas or want easier backups), set:

```bash
S3_ACCESS_KEY_ID=...
S3_SECRET_ACCESS_KEY=...
S3_BUCKET=campbooks-storage
S3_REGION=eu-central-1
# For non-AWS providers (MinIO, Hetzner, Backblaze B2, …):
S3_ENDPOINT=https://...
S3_FORCE_PATH_STYLE=true
```

### Search (OpenSearch)

OpenSearch is **optional** and only powers contact full-text autocomplete.
Email search, document search, and the Cmd+K palette all use Postgres + pgvector
and work without it; contact search falls back to SQL matching. To enable it:

```bash
# in .env
OPENSEARCH_URL=http://opensearch:9200
```
```bash
docker compose --profile search up -d
```

### Error monitoring & push

- `SENTRY_DSN` — a Sentry-compatible DSN (e.g. self-hosted GlitchTip). Disabled if unset.
- `APNS_*` / `FCM_*` — only needed if you build the native iOS/Android apps. Disabled if unset.

---

## Running in production

For anything beyond local use, terminate TLS with a reverse proxy and turn SSL on.

1. Point a domain at your server and set, in `.env`:
   ```bash
   APP_HOST=app.example.com
   FORCE_SSL=true
   WEB_PORT=3000
   ```
2. Put a TLS-terminating proxy in front. With [Caddy](https://caddyserver.com)
   (automatic HTTPS) it's just:
   ```
   app.example.com {
       reverse_proxy localhost:3000
   }
   ```
   The proxy speaks HTTPS to the world and forwards to the container on `:3000`;
   `FORCE_SSL=true` makes Rails emit https links, set secure cookies, and send
   HSTS. (The `/up` health endpoint stays reachable over plain HTTP for probes.)
3. `docker compose up -d --build`.

> **Don't expose the app over plain HTTP on the internet.** With `FORCE_SSL=false`
> sessions and cookies travel unencrypted. Always use a TLS proxy in production.

### Backups

Two things hold state: the Postgres volume and the uploads.

```bash
# Database (the app DB; cache/queue/cable are regenerable)
docker compose exec -T postgres pg_dump -U campbooks_app cb_primary | gzip > campbooks-$(date +%F).sql.gz

# Uploads (only if you use local disk storage, not S3)
docker run --rm -v campbooks_storage:/data -v "$PWD":/backup alpine \
  tar czf /backup/campbooks-storage-$(date +%F).tar.gz -C /data .
```

Store backups off the host. If you use S3 for storage, only the database needs backing up here.

### Upgrades

```bash
git pull
docker compose up -d --build      # the entrypoint runs migrations on boot
```

Back up the database first. Migrations run automatically when the `web`
container starts.

### Console & tasks

```bash
docker compose exec web bin/rails console
docker compose exec web bin/rails db:seed     # optional demo data / login
```

---

## Troubleshooting

**Every request redirects to `https://` and fails.** `FORCE_SSL` is on but
there's no TLS proxy. Set `FORCE_SSL=false` for local use, or put a TLS proxy in
front for production.

**"Blocked hosts" / 403 on every page.** `APP_HOST` doesn't match how you're
reaching the app. Set it to the exact hostname/IP in the URL bar and recreate the
containers.

**`web` keeps restarting with a database/extension error.** Make sure the
`postgres` service is the bundled `pgvector/pgvector` image (a vanilla `postgres`
image can't `CREATE EXTENSION vector`). If you swapped it, switch back.

**`worker` logs errors right after starting.** It waits for `web` to finish
migrations; transient errors during first boot settle once `web` is healthy.

**Missing required secret on `docker compose up`.** You skipped
`bin/generate-secrets`, or `.env` is missing a key. Re-run the script.
