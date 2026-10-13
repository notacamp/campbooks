# Changelog

All notable changes to Campbooks Next are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- Versioned releases of Next: bumping `VERSION` (with a rolled changelog
  section) on `next` publishes `ghcr.io/notacamp/campbooks:next-vX.Y.Z` and
  `:next-vX.Y` (amd64 + arm64, Trivy-scanned, with provenance and SBOM
  attestations), tags `next-vX.Y.Z` and creates a GitHub prerelease. See
  CONTRIBUTING.md → Releases.
- Foundation for the rewrite: Rust API skeleton (kernel, AI gateway, `system`
  module with health and version), one React/TypeScript client with a Tauri 2
  shell, OpenSpec workflow, architecture decision records, CI quality and
  security gates, and docker compose for self-hosting.
- The server image now serves the web app next to the API on the same port:
  set `CAMPBOOKS_WEB_DIR` to a built client (the image does this for you).
  Deep links fall back to the app, hashed assets are cached for a year, and
  every response carries security headers (a strict CSP on pages).
- Interim `/bff/api/*` passthrough to the API so the web app works from the
  image before the session-based BFF lands. It adds no credentials.
- Self-hosting: `docker compose up` now also starts the identity provider
  (Rauthy) on port 8081, with no setup. Its secrets are generated on first boot
  into the new `idp-data` volume, and Campbooks' scopes and sign-in clients are
  registered automatically. Read the first admin password with
  `docker compose run --rm idp-init cat /app/data/campbooks/initial-admin-password`;
  see `deploy/README.md`. Sign-in is not wired into the app yet.
- The API is now an OAuth 2.1 resource server. It validates JWT access tokens
  from your identity provider (signature against its JWKS, issuer, audience,
  expiry) and can require a scope on each endpoint. Rejected tokens get a 401,
  and valid tokens without the scope get a 403, both as problem+json with a
  `WWW-Authenticate` challenge. Self-hosters configure it with the new
  `CAMPBOOKS_AUTH__ISSUER`, `CAMPBOOKS_AUTH__AUDIENCE` (default
  `campbooks-api`), `CAMPBOOKS_AUTH__JWKS_URL` (discovered when unset) and
  `CAMPBOOKS_AUTH__LEEWAY_SECONDS` (default 60) settings. Without an issuer,
  scoped endpoints answer 401 `auth_not_configured`. Health and version stay
  public.
- `campbooks-server scopes` prints the API's scope catalogue as JSON, and the
  OpenAPI document declares a `bearer` security scheme and states every
  operation's security.
- Durable background work. Domain events are written to a retained event log
  in the same database transaction as the change they describe, and each
  module that subscribes gets its own delivery job, so nothing is lost on a
  restart. Jobs are retried with backoff, shared fairly between workspaces,
  handed to another worker if theirs dies, and failed jobs are kept for
  inspection. Everything lives in your Postgres (schema `kernel`); there is
  no broker to run.
- `campbooks-server` runs in roles: `all` (the API and the job worker in one
  process, the default and what the image runs), `api` and `worker`, so
  larger installs can scale the worker separately. `serve` still works as an
  alias of `all`.
- `campbooks-server events replay <consumer> [--since <time or event id>]`
  delivers logged events to one consumer again.
- New optional settings for the worker: `CAMPBOOKS_JOBS__CONCURRENCY`
  (default 4 jobs at once per process), `CAMPBOOKS_JOBS__PER_WORKSPACE`
  (2), `CAMPBOOKS_JOBS__POLL_INTERVAL_MS` (5000; new jobs also wake workers
  at once), `CAMPBOOKS_JOBS__LEASE_SECONDS` (60),
  `CAMPBOOKS_JOBS__RETENTION_HOURS` (168, how long succeeded jobs are kept),
  `CAMPBOOKS_JOBS__SHUTDOWN_GRACE_SECONDS` (8),
  `CAMPBOOKS_JOBS__RETRY_BASE_MS` (1000) and
  `CAMPBOOKS_JOBS__RETRY_MAX_SECONDS` (3600).
- The AI gateway's `decide` role is now typed (ADR 0009): modules ask
  `choice`, `score` and yes/no questions about an item and get answers with a
  probability per option and a confidence, which they must clear before
  acting. It is served by Laya, an open-weight decision model, or by any
  instruct model through the OpenAI-compatible API. New settings:
  `CAMPBOOKS_AI__DECIDE__PROTOCOL` (`laya` or `openai`, default `openai`, so
  existing `decide` settings keep working), `CAMPBOOKS_AI__DECIDE__LOGPROBS`
  (default `true`; set it to `false` for servers that reject the parameter)
  and `CAMPBOOKS_AI__DECIDE__CALIBRATION__<TYPE>[_<OPTIONS>]` temperatures
  (default 1.0). An empty `CAMPBOOKS_AI__<ROLE>__API_KEY` now means no key.
- Self-hosting: `docker compose --profile ai` now also runs `laya`, the
  default `decide` model (Laya multilingual, Apache-2.0, CPU-friendly). It is
  built locally from `deploy/laya`, downloads its checkpoint (about 680 MB)
  into the new `laya-models` volume on first start, is reachable only on the
  compose network, and refuses to start until you set
  `CAMPBOOKS_AI__DECIDE__API_KEY` in `.env` (`openssl rand -hex 32`).
  `.env.example` now points `decide` at it; see `deploy/README.md`.
- Workspaces. People who sign in get a Campbooks user; they create workspaces
  (business or personal, with an accent colour), invite others by email as
  admins or members, change roles, remove members or leave, and owners can
  delete a workspace: it disappears at once and its data is purged after 30
  days. A workspace always keeps at least one owner. Owners and admins can
  read a security log (sign-ins, role changes, invitations, removals,
  deletion). The API lives under `/api/v1/workspaces` and
  `/api/v1/w/{workspace_id}/…`, with new scopes `workspaces:read`,
  `workspaces:write` and `workspaces:admin` (registered in the bundled IdP;
  add them to sign-in clients provisioned earlier). Invitations are not
  emailed yet; set `CAMPBOOKS_ENV=development` to have the server log the
  link.
- `CAMPBOOKS_SIGNUP__POLICY` decides who may create a first workspace:
  `open` (default), `invite_only`, `approval` (requests are recorded and
  refused for now) or `code` (with `CAMPBOOKS_SIGNUP__CODES`). The instance's
  first user can always create one.
- Each workspace's data is also isolated by Postgres row-level security. The
  server now runs migrations as your database user and then queries as the
  role `campbooks_app` (created by the migrations; `CAMPBOOKS_DATABASE_ROLE`
  to change it), and warns at startup if that role would bypass the
  policies. Nothing to do with the bundled Postgres; on a managed Postgres
  whose user cannot create roles, see `deploy/README.md`.
- API errors for malformed JSON bodies and query strings are now
  problem+json 400s, and unparsable ids in paths are 404s.

### Changed
- The image's default command is now `all` instead of `serve` (same
  behaviour plus the job worker). Each job the worker runs may hold a
  database connection: keep `CAMPBOOKS_DATABASE_MAX_CONNECTIONS` (default 10)
  above `CAMPBOOKS_JOBS__CONCURRENCY`.
