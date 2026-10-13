# Changelog

All notable changes to Campbooks Next are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- Foundation for the rewrite: Rust API skeleton (kernel, AI gateway, `system`
  module with health and version), one React/TypeScript client with a Tauri 2
  shell, OpenSpec workflow, architecture decision records, CI quality and
  security gates, and docker compose for self-hosting.
- The server image now serves the web app next to the API on the same port:
  set `CAMPBOOKS_WEB_DIR` to a built client (the image does this for you).
  Deep links fall back to the app, hashed assets are cached for a year, and
  every response carries security headers (a strict CSP on pages).
- Self-hosting: `docker compose up` now also starts the identity provider
  (Rauthy) on port 8081, with no setup. Its secrets are generated on first boot
  into the new `idp-data` volume, and Campbooks' scopes and sign-in clients are
  registered automatically. Read the first admin password with
  `docker compose run --rm idp-init cat /app/data/campbooks/initial-admin-password`;
  see `deploy/README.md`.
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
- Web sign-in. The web app now signs you in through the identity provider,
  and your browser never holds a token: the server keeps them (encrypted, in
  Postgres schema `bff`) and gives the browser a single `HttpOnly`,
  `SameSite=Strict` session cookie. Sessions end after 48 hours unused or 30
  days at most, on sign-out, or when you sign out at the identity provider
  (over https). Opening the app while signed out sends you to sign in and
  back to the same page. New settings: `CAMPBOOKS_SECRET_KEY` /
  `CAMPBOOKS_SECRET_KEY_FILE` (required once sign-in is configured; docker
  compose generates one for you into the new `server-secrets` volume) and the
  optional `CAMPBOOKS_BFF__*` block in `deploy/.env.example`. Changing the
  secret key signs everyone out of the web app and loses nothing else.
- `docker compose up` starts a one-shot `server-init` service that prepares the
  `server-secrets` volume; the web client's sign-in secret is now saved there
  (it used to be in `idp-data`).

### Changed
- `/bff/api/*` now requires a signed-in session and calls the API with your
  access token; without a session it answers 401 `session_required`. It
  replaces the interim passthrough.
- Request logs record the path of each request but no longer its query
  string, which can carry secrets.
- The image's default command is now `all` instead of `serve` (same
  behaviour plus the job worker). Each job the worker runs may hold a
  database connection: keep `CAMPBOOKS_DATABASE_MAX_CONNECTIONS` (default 10)
  above `CAMPBOOKS_JOBS__CONCURRENCY`.
