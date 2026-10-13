# Changelog

All notable changes to Campbooks Next are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- The server now answers in your language: error messages from the API are
  in English, European Portuguese, Spanish or French, following the language
  you chose (once accounts can save one) or your browser's language, and say
  which one they use in `Content-Language`. Translations are checked in CI
  (`cargo xtask i18n`).
- Design system for the new app: light, dark and system themes; three
  workspace accents (business in ink, personal in terracotta, forest) and a
  compact density, remembered on each device and applied before the page
  first paints; Funnel Sans and Funnel Display, self-hosted; and the shared
  components the new screens are built from (buttons, keyboard hints,
  avatars, signals, the assistant's visor and note, toasts with undo, empty
  and loading states, queue rows, conversation rows, navigation, tab bar).
- Keyboard shortcuts everywhere, shown on the buttons and rows they trigger:
  G then a letter to move between places, J/K/Enter in lists, E, L, R, H on
  the selected item, Z to undo, ? for the full list, and ⌘K (Ctrl+K) to search
  every command. Shortcuts never fire while you type in a field.
- Storybook for the design system (`pnpm --dir clients storybook`), with the
  approved Home, Inbox, phone, states and themes screens on demo data, checked
  for accessibility in light and dark on every pull request.
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
- Storage for message bodies, attachments and files, outside Postgres. Each
  file is stored once per workspace however many messages carry it, checked
  against its SHA-256 on the way in and out, and deleted by an hourly sweep
  once nothing uses it and `CAMPBOOKS_BLOBS__SWEEP_GRACE_HOURS` (default 24)
  have passed. By default files go to the new `blobs-data` volume
  (`CAMPBOOKS_BLOBS__PATH`, `/data/blobs`); set
  `CAMPBOOKS_BLOBS__BACKEND=s3` and `CAMPBOOKS_BLOBS__S3__*` to use any
  S3-compatible bucket instead. Files over `CAMPBOOKS_BLOBS__MAX_OBJECT_BYTES`
  (default 50 MiB) are refused. The server checks the store on startup and
  will not start if it cannot reach it. Back up `blobs-data` (or the bucket)
  together with Postgres; see `deploy/README.md`.
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
- Every API operation is now an entry of one catalogue, the OpenAPI document
  (ADR 0013): each has a stable `operationId` (`<module>.<verb>`, for example
  `system.health`; the old `system_health`-style ids are gone) and an
  `x-campbooks` extension saying whether it changes data, which operation
  undoes it, whether automation rules and the assistant may call it, its risk
  and the record types it touches. Domain events are listed the same way in
  the new `openapi/events.json` (`campbooks-server events catalogue` prints
  it).
- Mutating API calls accept an `Idempotency-Key` header: retrying a request
  with the same key returns the first response (marked
  `Idempotency-Replayed: true`) instead of running it twice. Reusing a key
  for a different request answers 422, and a key whose first request is still
  running answers 409. Keys are kept for 24 hours, in the new
  `kernel.idempotency` table (created by the migration that runs on startup),
  and an hourly job deletes expired ones.
- Self-hosting: `docker compose --profile ai` now also runs `laya`, the
  default `decide` model (Laya multilingual, Apache-2.0, CPU-friendly). It is
  built locally from `deploy/laya`, downloads its checkpoint (about 680 MB)
  into the new `laya-models` volume on first start, is reachable only on the
  compose network, and refuses to start until you set
  `CAMPBOOKS_AI__DECIDE__API_KEY` in `.env` (`openssl rand -hex 32`).
  `.env.example` now points `decide` at it; see `deploy/README.md`.
- Contributor tooling: `cargo xtask new-module <name>` scaffolds a module
  (crate, scopes, events, a scoped read slice with tests, schema migration,
  OpenSpec change, client module with its four locale files) and registers it
  in the workspace, the server and the client shell; `cargo xtask new-feature
  <module> <feature>` adds a slice to a module; `cargo xtask scaffold-check`
  (also run in CI) proves their output passes the gates. Optional local git
  hooks (`lefthook install`), Claude Code skills for starting a task,
  developing a feature and adding a module, and a guide to working in
  parallel worktrees (`docs/parallel-work.md`).
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
- **API clients:** every problem+json error now carries a stable `code`
  (`not_found`, `invalid_token`, `last_owner`, …); `detail` is now a
  sentence in the caller's language instead of that code. Match on `code`.
- A user's saved language (`workspaces.users.locale`) is now empty until
  they choose one, so the browser's language applies meanwhile. The
  migration runs on startup; nothing to do.
- The image's default command is now `all` instead of `serve` (same
  behaviour plus the job worker). Each job the worker runs may hold a
  database connection: keep `CAMPBOOKS_DATABASE_MAX_CONNECTIONS` (default 10)
  above `CAMPBOOKS_JOBS__CONCURRENCY`.
