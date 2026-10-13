# Changelog

All notable changes to Campbooks Next are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Fixed
- `CAMPBOOKS_ZOHO__ACCOUNTS_SERVER` (default `https://accounts.zoho.com`): the Zoho
  accounts server sign-in sends people to. A Zoho app registered in one data centre
  only (for example the EU console) is rejected as an invalid client by
  `accounts.zoho.com`; set this to your app's data centre (`https://accounts.zoho.eu`).
  It must be one of Zoho's accounts servers, or Campbooks refuses to start.

### Added
- Passkeys. Add one under Settings → Ways to sign in (Add a passkey, `N` `P`), then sign
  in with it on the sign-in screen: the passkey button (`P`) or your browser's
  autofill. You can also create an account with a passkey alone, no Google, Microsoft or
  Zoho needed (not on an invite-only instance, where an invitation is matched by email
  address). Passkeys are ways in like provider accounts: rename or remove them (never your
  last way in; removing one signs out whoever used it), and adding or removing one needs
  a sign-in in the last ten minutes. They need `https://` (or `http://localhost`), and are
  bound to the host of `CAMPBOOKS_PUBLIC_URL`: changing that host retires them.
- Attachments open from the reader. Each message lists its files with their
  size, Download and, for pictures and PDFs, Preview: a picture opens in a
  dialog, a PDF in your browser's own viewer in a new tab. V moves through
  the attachments, Enter opens one. HTML, SVG and every other type only
  download. The new operation `conversations.get_attachment` (scope
  `conversations:read`) serves the bytes as a download under a cleaned file
  name, with the stored type only when it is on a safe list (pictures and
  PDFs must also start like one) and a `sandbox` policy. Pictures inside a message's body (`cid:`) now show without asking,
  since nothing remote loads; remote images that stay hidden show a quiet
  placeholder instead of a broken-image icon. Mail synced before this
  release keeps the placeholder for its inline pictures. The web client's
  Content-Security-Policy now allows `blob:` images (the picture preview).
- Reply, reply all, forward and new messages, sent through your Gmail. The
  composer opens under the thread (R, A, F) or on its own screen (C): From
  your mailbox, To, Cc and Bcc as chips, the quoted history folded under your
  text, forwarded attachments you can drop, and plain text with basic
  formatting (⌘B, ⌘I, lists). Drafts save as you type and come back when you
  reopen the conversation. ⌘↵ sends after a 10-second window in which Undo
  takes it back (`CAMPBOOKS_CONVERSATIONS__UNDO_SEND_SECONDS`, 0 to 60). A
  send never goes out twice, even after a timeout or a restart, and the sent
  message appears in its conversation at once. Sending needs the `sender`
  or `manager` role on the inbox and the new `conversations:send` scope,
  which the web app asks for by itself; mailboxes already connected need no
  new consent.
- What you do to mail in Campbooks now happens in Gmail too: marking read or
  unread, starring, archiving and moving back to the inbox, adding or
  removing a Gmail label, and moving a conversation to Primary, Updates or
  Newsletters (Gmail's Primary, Updates and Promotions tabs). Changes reach
  Gmail on the mailbox's next sync, normally within a second, and never come
  back as new changes. If the same thing was changed in Gmail after you
  changed it in Campbooks, the later change wins and Campbooks shows it.
  Throttling and outages are retried with backoff within the shared Google
  quota; a change Gmail refuses is recorded (without any message content)
  and Campbooks shows the message as Gmail has it. A change that could not
  reach Gmail within 7 days (a long pause, a reconnect) is dropped rather
  than made late, mail in Gmail's trash or spam is never brought back to the
  inbox, and only read, star, inbox, category and your own labels are ever
  touched. Labels you create in
  Campbooks and the Receipts stream stay in Campbooks for now. New table
  `connectors.outbound_changes` (migration applied on start).
- The inbox: the four streams (Primary, Updates, Receipts, Newsletters) under
  Inbox with their unread counts, each stream's conversations newest first,
  and a reader with the thread (older messages folded, attachments listed).
  Campbooks' sorting shows only as a suggestion ("Suggests Newsletters"),
  with "Why?" listing the stored reasons and "Move to …" one key away; Done,
  star, read/unread and move all have shortcuts and Undo. On phones the list
  and the reader are separate screens. New read operation
  `conversations.list_streams`; conversation summaries carry `suggestion`
  and details carry `decisions`.
- Email bodies are sanitised on the server before they reach the app
  (scripts, style sheets, forms, frames and tracking pixels removed, links
  open in a new tab), shown only in a sandboxed frame, and remote images stay
  blocked until you choose "Show images" for a message.
- Instance administrators (ADR 0015). An operator makes someone an
  administrator from the server's command line,
  `campbooks-server admin grant --email you@example.com --permission super`
  (`docker compose exec server …`; also `revoke`, `list` and `log`), never
  from the app or the API. Administrators get an **Admin** section in the
  app (account menu → Admin) with a System panel: version, uptime, database
  and migrations, the effective configuration without any secret, and
  whether each AI role's provider answers. The admin API is separate
  (`/admin/v1/*`, `openapi/admin.json`), answers 404 to everyone else, asks
  for a sign-in less than 12 hours old, and records every request in an
  audit trail. A new `admin` database schema is created by the migrations.
  See `deploy/README.md` → Administrators.
- The sign-in screens are Campbooks screens, in English, Portuguese, Spanish
  and French. Opening the app signed out goes straight to "Sign in to
  Campbooks" (and back to the page you opened afterwards), with one button
  per way in this instance offers (keys `1`, `2`, `3`; `P` for a passkey once
  passkeys arrive). Before you leave for Google it says what the one consent
  covers (your Gmail is connected too, what Campbooks keeps, that you can
  untick mail access) and that Google may warn the app isn't verified yet.
  A first sign-in shows the new-account screen (your name and address, and
  what happens next); every sign-in problem is explained with what to do.
- Right after you create your first workspace, "Your Gmail … will be
  connected to this workspace": Connect, or Not now. Home keeps offering it
  (Connect, or forget it) until it expires after 7 days, and names the
  mailbox once it is connected.
- Sign in with Microsoft (personal, work and school accounts;
  `CAMPBOOKS_MICROSOFT__CLIENT_ID`, `__CLIENT_SECRET_FILE`, `__TENANT`, default
  `common`) and with Zoho in every data centre (`CAMPBOOKS_ZOHO__CLIENT_ID`,
  `__CLIENT_SECRET_FILE`), when Campbooks is its own identity provider. Both take
  `__REDIRECT_PATH` like Google. Register `/auth/callback/microsoft` and
  `/auth/callback/zoho` (see `deploy/README.md` → Built-in sign-in).
- More ways in: Settings → Ways to sign in lists how you sign in, adds another
  Google, Microsoft or Zoho account, and removes one (never the last). Every
  mailbox you connect becomes a way in too. A provider account that is already
  another Campbooks account's way in is refused, never merged. Adding or removing
  a way in, or connecting a mailbox, needs a sign-in in the last ten minutes.
  Removing a way in signs out every session that signed in with it.
- `CAMPBOOKS_TRUSTED_PROXIES` (comma-separated addresses or CIDR blocks): the
  reverse proxies whose `X-Forwarded-For` names the client. Set it behind a proxy,
  or the per-address sign-in rate limits count every client as one (see
  `deploy/README.md` → Built-in sign-in).
- `CAMPBOOKS_GOOGLE__REDIRECT_PATH` (default `/auth/callback/google`): the callback
  path Campbooks sends to Google when your OAuth app registered another one, with a
  reverse proxy rewriting it to `/auth/callback/google`. The same
  `CAMPBOOKS_<PROVIDER>__REDIRECT_PATH` pattern applies to future sign-in providers.
- Campbooks can be its own identity provider (ADR 0014), replacing Rauthy in
  a later release. With `CAMPBOOKS_AUTH__ISSUER` unset and
  `CAMPBOOKS_SECRET_KEY` set, the server is an OAuth 2.1 / OpenID Connect
  issuer on its own origin (`/.well-known/openid-configuration`,
  `/oauth2/*`): Ed25519 signing keys generated, encrypted at rest and
  rotated automatically, authorization code with PKCE only, refresh tokens
  that rotate and revoke the whole sign-in when one is reused, and the web
  app signing in through it with nothing to provision. People sign in with
  Google; a new person's mailbox comes with the same consent and is
  connected when they create their first workspace. Accounts are linked to
  a Google account by its id, never by email address. Add
  `${CAMPBOOKS_PUBLIC_URL}/auth/callback/google` to your Google OAuth client
  (see `deploy/README.md` → Built-in sign-in). The shipped `compose.yaml`
  still uses Rauthy, so existing installs see no change yet; the sign-in
  screens arrive in the next release. The built-in issuer needs an https
  `CAMPBOOKS_PUBLIC_URL` (http only with `CAMPBOOKS_ENV=development`), refuses
  to start if `CAMPBOOKS_SECRET_KEY` cannot open its stored signing keys,
  and on an `invite_only` instance creates accounts only for invited
  addresses.
- The new app's first run: a calm signed-out page with one "Sign in" (that
  brings you back to the page you opened), then creating your first
  workspace one question at a time (its name, then Business or Personal,
  which picks its colour), with Skip where a default works. Instances that
  require a signup code, an invitation or an administrator's approval each
  say so plainly.
- Workspaces in the address (`/w/<id>/…`), a sidebar (a header and tab bar
  on phones), the workspace switcher (⌘⇧W, Ctrl+Shift+W elsewhere) with each
  workspace in its own colour, and an account menu with theme, density,
  language and Sign out. The app reopens the workspace you used last on that
  device.
- An empty Home ("Nothing needs you yet") with the shortcuts that work today;
  "Connect your mail" says it is coming, because mail connections are not
  built yet.
- Settings → Members: owners and admins change roles (with Undo), remove
  members after confirming in place, invite people by email and see pending
  invitations. Campbooks does not email invitations yet, so right after
  inviting you get the invitation link once, with a Copy button (C), to send
  it on yourself; inviting again gives a fresh link. The link opens a page
  that signs you in if needed and joins the workspace.
- Conversations, the first piece of the inbox: inboxes, conversations and
  messages (email first), threaded by the provider's thread or by replies,
  with bodies and attachments kept in blob storage. Conversations are sorted
  into Primary, Updates, Receipts and Newsletters, and can be read, marked
  read or unread, starred, archived, labelled and moved to another stream
  through the API; a move is remembered as a correction. Connecting a Gmail
  account creates its inbox (the member who connected it manages it) and
  fills it with the synced mail; other members see an inbox once they are
  added to it, and only those who can send from it can change its
  conversations. Conversation changes reach the live updates stream, shown
  only to the inbox's members.
- Every new message gets the AI pass when the `decide` role is configured;
  its answers are recorded as suggestions only and change nothing yet. Mail
  older than `CAMPBOOKS_CONVERSATIONS__DECIDE_BACKFILL_DAYS` (default 14) gets
  no AI pass, so importing an old mailbox costs no AI.
- Connect a Gmail account (API only for now; the screen comes next): Campbooks
  asks Google for consent, keeps the grant encrypted with the instance secret
  key, reads the last 90 days of mail and then follows new mail and label
  changes, every minute while the mailbox is busy and backing off to every few
  hours when it is quiet. Each connection shows its health (ok, needs
  reauthorisation, throttled, failing); disconnecting, leaving the workspace or
  deleting it revokes the grant at Google. Off until you register a Google
  OAuth client (`CAMPBOOKS_GOOGLE__CLIENT_ID`,
  `CAMPBOOKS_GOOGLE__CLIENT_SECRET_FILE`; see `deploy/README.md` → Connected
  accounts); tuning is `CAMPBOOKS_CONNECTORS__*`, including the largest
  message kept (`…__GMAIL__MAX_MESSAGE_BYTES`, 50 MiB) and the bytes one sync
  run may download (`…__GMAIL__RUN_BUDGET_BYTES`, 64 MiB). The web app's
  callback screen at `/connections/google/callback` is not built yet, so
  connecting goes through the API for now. Note: replacing the instance
  secret key now also asks members to connect their accounts again.
- Live updates: the app learns what changed in a workspace (renamed, a
  member joined, left or changed role) as it happens and refreshes only what
  it shows, catching up after a lost connection or a sleeping laptop. API
  clients can follow the same stream at `GET /api/v1/w/{workspace_id}/changes`
  (Server-Sent Events, ids only). Self-hosters behind a reverse proxy must
  let `text/event-stream` responses through unbuffered; tuning lives in the
  new `CAMPBOOKS_CHANGES__*` settings (see `deploy/.env.example`).
- Sign in with Google, Microsoft or Zoho. Set a provider's client ID in
  `.env` (`CAMPBOOKS_IDP_GOOGLE_CLIENT_ID`, `CAMPBOOKS_IDP_MICROSOFT_CLIENT_ID`,
  `CAMPBOOKS_IDP_ZOHO_CLIENT_ID`) and put its secret in
  `deploy/secrets/<provider>-client-secret`; `idp-provision` registers it in
  the identity provider and the login page shows its button. Whether a
  provider may create accounts for new people follows
  `CAMPBOOKS_SIGNUP__POLICY` (not under `invite_only`), overridable with
  `CAMPBOOKS_IDP_PROVIDER_SIGNUP`. Microsoft's tenant
  (`CAMPBOOKS_IDP_MICROSOFT_TENANT`, default `common`) and Zoho's data centre
  (`CAMPBOOKS_IDP_ZOHO_ISSUER`, default `https://accounts.zoho.eu`) are
  settings. Existing installs grant the IdP's `campbooks` API key the
  "Auth Providers" right once in the admin UI before enabling a provider. See
  `deploy/README.md` → "Sign in with Google, Microsoft or Zoho".
- A way to measure the AI's sorting before it acts: a fictional, labelled set
  of 320 emails in English, Portuguese, Spanish and French
  (`crates/ai/evals/decide/mail-triage/`, with its labelling guide), and
  `cargo xtask eval decide`, which reports for each triage question (stream,
  needs a reply, asks you to do something, deadline, urgency, suspicious) how
  often each strategy is right, how much mail it could handle on its own at
  the confidence needed, its calibration and its speed. Model strategies run
  against any endpoint set in `CAMPBOOKS_EVAL_LAYA_URL`,
  `CAMPBOOKS_EVAL_INSTRUCT_URL` or `CAMPBOOKS_EVAL_EMBED_URL` and are reported
  as not run otherwise.
- Deterministic email signals, learning from your own corrections (nearest
  neighbours) and cascades of them behind one interface with the `decide`
  model, each answer carrying the reasons behind it.
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
- Monitoring, off unless you turn it on. `CAMPBOOKS_TELEMETRY__OTLP_ENDPOINT`
  sends traces and metrics over OTLP/HTTP to a collector (with
  `CAMPBOOKS_TELEMETRY__OTLP_HEADERS` for its credentials and
  `CAMPBOOKS_TELEMETRY__SAMPLE_RATIO`, default 1, for the share of traces
  kept). `CAMPBOOKS_TELEMETRY__METRICS_ADDR` serves Prometheus metrics at
  `/metrics` on a separate port: keep it private. Metrics cover request
  rates, errors (by their stable `code`) and latency per route, job durations and failures, queue
  depth and wait, AI call latency and tokens per role, the database pool and
  the build. Traces follow a request into the jobs it enqueues and the calls
  it makes to AI providers and the identity provider, and honour an incoming
  W3C `traceparent`. No message content, token, cookie or email address is
  ever recorded; a test enforces it. See `deploy/README.md` → Monitoring.

### Changed
- The members page's "copy invitation link" shortcut is now `Y` (`C` writes a
  new message everywhere).
- The web app's Content-Security-Policy now allows inline `style` attributes
  (`style-src-attr 'unsafe-inline'`) and `https:` images, which sanitised
  email bodies need inside their sandboxed frame; scripts, style sheets and
  connections stay same-origin. Reverse proxies that set their own CSP need
  the same two additions.
- Connecting a mailbox goes through the built-in sign-in: Google redirects to
  `/auth/callback/google`, and `/connections/google/callback` is no longer used
  (remove it from your Google OAuth client). `connectors.complete_connection` is
  gone; `connectors.start_connection` answers with the sign-in URL. An install still
  on Rauthy cannot connect mail until it switches to the built-in sign-in.
- **API clients:** `POST /api/v1/w/{workspace_id}/invitations` now answers
  with `accept_url`, the invitation's accept link, built from
  `CAMPBOOKS_PUBLIC_URL`. It is returned only in that response (never listed,
  logged or sent in an event); inviting the same address again returns a
  fresh link. Development servers no longer log invitation links.
- **API clients:** every problem+json error now carries a stable `code`
  (`not_found`, `invalid_token`, `last_owner`, …); `detail` is now a
  sentence in the caller's language instead of that code. Match on `code`.
- A user's saved language (`workspaces.users.locale`) is now empty until
  they choose one, so the browser's language applies meanwhile. The
  migration runs on startup; nothing to do.
- **Separate database logins.** The server now queries as a dedicated
  non-superuser login (`campbooks_runtime`) that cannot bypass row-level
  security or switch back to a more powerful role, and the owner login is used
  only to run migrations. New settings: `CAMPBOOKS_DATABASE_MIGRATION_URL`,
  `CAMPBOOKS_DATABASE_PASSWORD_FILE` and `CAMPBOOKS_DATABASE_ALLOW_RLS_BYPASS`.
  **Upgrading with the bundled compose file needs no manual step**: the new
  `db-init` service creates the login and its password on the first
  `docker compose up -d` (existing data is untouched). With your own
  Postgres nothing breaks either: a single privileged `CAMPBOOKS_DATABASE_URL`
  keeps working, with a startup warning recommending the split (see "Database
  logins" in `deploy/README.md`). The server now refuses to start when the
  login it queries as is a superuser or has `BYPASSRLS` (for example with
  `CAMPBOOKS_DATABASE_ROLE=` set empty on a superuser URL); set
  `CAMPBOOKS_DATABASE_ALLOW_RLS_BYPASS=true` to opt out.
- `/bff/api/*` now requires a signed-in session and calls the API with your
  access token; without a session it answers 401 `session_required`. It
  replaces the interim passthrough.
- Log lines inside a request or a job now carry the W3C `trace_id`, and the
  request's span records the route template (`http.route`) instead of the
  full URI, which could carry personal data in its query string. A
  client-sent `x-request-id` that is not a plain token is logged as
  `invalid`. The request log line is now `request finished`.
- The image's default command is now `all` instead of `serve` (same
  behaviour plus the job worker). Each job the worker runs may hold a
  database connection: keep `CAMPBOOKS_DATABASE_MAX_CONNECTIONS` (default 10)
  above `CAMPBOOKS_JOBS__CONCURRENCY`.
