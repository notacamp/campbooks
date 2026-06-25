# Campbooks

Not A Camp's AI-native email client — an inbox that sorts itself, for young professionals and small-business owners drowning in email and paperwork. Rails 8.1, PostgreSQL, Tailwind CSS 4, Hotwire, Solid Queue, Phlex components.

## 🔒 PUBLIC REPO — KEEP IT CLEAN (READ FIRST)

**This repository (`notacamp/campbooks`) is the PUBLIC, source-available core (Sustainable Use License). NOTHING private, company-specific, or secret may live here — not in the code, the docs, OR the git history.** Anything private goes in the **private `notacamp/campbooks-cloud`** repo instead.

- **Private stuff → `campbooks-cloud`, never here:** the production deploy pipeline, the secrets-management / Infisical runbook, the security pentest, app-store submission docs, server IPs / SSH / host details, and the private ops Claude context (`CLAUDE.cloud.md`). `campbooks-cloud/install.sh` overlays that onto a local checkout by writing a **gitignored `CLAUDE.local.md`** that `@`-imports the private context — so Claude sees ops info locally while this repo stays clean. **If you're about to add anything with an IP, hostname, secret name, account ID, or a real person's name/email, it belongs in `campbooks-cloud`.**
- **Genericize in this repo:** use `ENV` with neutral `example.com` defaults; no hardcoded infra. The domain `not-a-camp.com` and the product name "Not A Camp" are fine (public product). ⚠️ The GitHub org is **`notacamp`** (no hyphens) — distinct from the domain.
- **History is intentionally FRESH:** the public repo is a single "initial public release" commit because the original dev history contained sensitive files. The full history lives ONLY in the private archive remote (`legacy-private` → the old personal repo). **Never push a full-history branch to `origin`** — only ever publish clean history.
- **Deploy:** publishing a GitHub **release** (`vX.Y.Z`) → `.github/workflows/publish-image.yml` re-runs the test gate, builds the multi-arch image, pushes it to GHCR, then fires a `repository_dispatch` → `campbooks-cloud`'s `deploy.yml` **pulls that image tag** and recreates the prod containers. **Merges to `main` no longer deploy on their own**; roll back by redeploying a prior version (`workflow_dispatch`). Secrets: `CAMPBOOKS_CLOUD_DISPATCH_TOKEN` lives here; `DEPLOY_SSH_KEY` lives in `campbooks-cloud`.

**Positioning & voice:** the canonical product positioning, USPs, and vocabulary live in `docs/messaging.md` — keep all user-facing copy (app, website, in-repo docs) consistent with it. Aim for consistency, not uniformity: never contradict it, but don't robotically repeat the same lines either.

## Contributing & development workflow

Full guide for both human contributors and AI agents: **`CONTRIBUTING.md`**. The essentials:

- **Branch + PR, never commit straight to `main`.** `main` is protected; merges don't deploy on their own — **publishing a release ships to prod** (builds the image, then deploys it). The bar to merge is still "production-ready" — `main` should always be releasable. Branch off `main` (`feat/…`, `fix/…`, `docs/…`, `refactor/…`, `chore/…`), open a PR, get CI green, squash-merge.
- **PR title is a Conventional Commit** (`feat:`, `fix:`, `docs:`, `refactor:`, `chore:`; `!` or `BREAKING CHANGE` for breaks) — the squash-merge uses the **PR title** as the commit on `main`, so it's what's strict; individual branch commits can be free-form.
- **`main` requires signed commits** (branch protection enforces verified signatures) — set up SSH/GPG signing once; see `CONTRIBUTING.md` → *Signed commits*.
- **CI gates (must pass before merge):** `bin/rubocop`, `bin/brakeman --no-pager`, `bin/bundler-audit`, `bin/importmap audit`, `bin/rails db:test:prepare test`. Run them locally first; keep i18n at parity (`bundle exec i18n-tasks missing`).
- **Update `CHANGELOG.md`** (Keep a Changelog format) under `[Unreleased]` for any user-visible change.
- **Semver:** the **`VERSION`** file at the repo root is the single source of truth (also reported at `/up` and in the Settings sidebar; constant is `Campbooks::VERSION`). Releases are `vX.Y.Z` git tags + a `CHANGELOG.md` section + a GitHub Release. MAJOR = breaks self-hosters/config/API; MINOR = backward-compatible feature; PATCH = fix. Pre-1.0, a minor may still break — call it out.
- **AI agents follow the same rules as humans**, plus: always branch (never `main`), Conventional PR title, update the changelog, run the gates and verify UI in a browser (Playwright, **375px + desktop**) before reporting done — and above all keep this public repo clean (no secrets/IPs/hostnames/account IDs/real names; see the section above). Details: `CONTRIBUTING.md` → *Working as an AI agent*.

## UI Components

- **All new UI must use Phlex components from `app/components/campbooks/`.** Do not write raw HTML with Tailwind classes in views when a component can cover the pattern.
- **Before writing HTML, check if a component exists for that pattern.** If the pattern is reusable but no component exists yet, extract it into a component first, then use it.
- **Every component needs a Lookbook preview** in `spec/components/previews/`. All variants and sizes must be shown.
- **Use the `impeccable` skill** when designing a new component, making visual changes to an existing one, or reviewing UI for consistency.
- **Component guidelines**: full conventions, naming, variant/size patterns, slot usage, and accessibility rules live in `docs/components.md`.
- **Coordination**: the `ui-refactor/` directory holds per-feature MD files that catalog which views still need component extraction. Builders read from these files and mark items done.

## Mobile responsiveness

- **The app must stay fully mobile responsive — no horizontal overflow down to 375px.** Verify every UI change at mobile width with Playwright before reporting done, not just desktop.
- **Mobile-first**: base classes target mobile; add `sm:`/`md:`/`lg:` to restore the wider layout (e.g. `grid-cols-1 sm:grid-cols-2 lg:grid-cols-3`). Never drop desktop behavior when fixing mobile.
- **Quick rules**: tables go in `overflow-x-auto` (never `overflow-hidden`); page headers/toolbars stack (`flex-col sm:flex-row`) with `flex-wrap` actions; side-by-side panes use `flex-col lg:flex-row`; fixed widths get a `w-full sm:` base.
- **Shell breakpoints**: topbar switches to the hamburger (`mobile-menu` controller) below `md`; the email/Scout multi-pane shells (`email_messages/*`, `agent_chat`) collapse to a single pane below `lg` via the `email-mobile` / `scout-mobile` controllers; settings sidebar stacks below `lg`.
- **Full patterns, breakpoints, and helper controllers** are documented under "Responsive design" in `docs/components.md`.

## Data safety

- **Never delete user data without explicit confirmation.** When cleaning up test data, use targeted deletion (by ID or known test attributes) rather than `delete_all`. Check with the user before running destructive queries on any table that may contain real records.
- Email accounts (`EmailAccount`) store OAuth refresh tokens. If these are deleted, the user must re-authenticate through the Zoho OAuth flow.

## Authentication

The app uses cookie-based sessions (`Authentication` concern). All controllers require authentication unless they call `allow_unauthenticated_access`.

**Seed credentials** (created by `db/seeds.rb`):
- `admin@example.com` / `changeme123`
- `partner@example.com` / `changeme123`

Login form is at `/session/new`. Sessions are stored in the `sessions` table with a signed cookie (`cookies.signed[:session_id]`).

When browser-testing with Playwright: navigate to `/session/new`, fill the email/password fields, click "Sign in", then navigate to the target page.

### Registration & signup modes

Self-serve registration lives in `RegistrationsController` (3 steps: name/email → 6-digit OTP → password → new `Workspace`). Who may create a brand-new account is controlled by `config.signup_mode` (set in `config/initializers/registration.rb` from `SIGNUP_MODE`; default `open` self-hosted / `beta_code` cloud):

- **open** — anyone with a valid email signs up.
- **beta_code** — anyone who also enters a valid single-use invite code (the current closed-beta default). Codes are `BetaCode` records minted by admins at `/admin/beta_codes`; validated at step 1, carried in the registration session, and redeemed atomically when the account is created.
- **approval** — signups create a pending `SignupRequest` for an admin to approve at `/admin/signup_requests` (the original gated beta).
- **invite_only** — no public signup; an `Invitation` is required.

Invited users always bypass the gate. Helpers `signup_mode`, `public_signup_allowed?`, and `beta_code_required?` are available in controllers and views. The cloud "Beta" badge (topbar + auth pages) is gated on `!self_hosted?`.

## Running the app

```
bin/rails server     # web server on :3000
bin/rails solid_queue:start   # job worker (needed for email scanning, recurring tasks)
```

Or add `worker: bin/rails solid_queue:start` to Procfile.dev.

## Email pipeline

- **OAuth flow**: `/email_accounts/new` → Zoho consent → callback at `/oauth/zoho/callback` → stores refresh token in `EmailAccount` (encrypted via `ActiveRecord::Encryption`)
- **Scanning**: `EmailScanJob` fetches all inbox messages via Zoho REST API, creates `EmailMessage` records, enqueues `EmailProcessJob` per message
- **Processing**: `EmailProcessJob` downloads attachments, creates `Document` records with `source: :email`, runs `Documents::Processor` (AI analysis + PDF conversion)
- **Deduplication**: composite unique index on `email_messages(email_account_id, zoho_message_id)`
- **Views**: `/email_accounts` (connect/status), `/email_scans` (audit trail), `/email_messages` (all ingested emails)
- **Mission Control**: Solid Queue dashboard at `/jobs`, gated to **admins** via `MissionControlController` (app session auth + admin check, wired through `config.mission_control.jobs.base_controller_class`; the engine's own HTTP basic auth is disabled)

## Calendar

Two-way calendar sync that rides on the **same OAuth grant as the mailbox** — connecting a Google/Zoho email account requests calendar scopes too and auto-provisions its calendar. There is no separate calendar connect flow.

- **Connect**: account-link consent (`EmailAccountsController#create`) requests calendar scopes (`Google::OauthClient::CONNECT_SCOPES`; the Zoho scope string includes `ZohoCalendar.*`) — **sign-in stays mail-only**. The mail callbacks (`Oauth::GoogleMailController`, `Oauth::ZohoController`) call `Calendars::AccountProvisioner` to create a `CalendarAccount` sharing the same refresh token, then enqueue a full sync. Reuses `/oauth/gmail|zoho/callback` (no new redirect URIs). Existing accounts must reconnect once to grant the scope.
- **Model**: `CalendarAccount` → `Calendar` (one per provider calendar; `syncing` toggle; holds the sync token) → `CalendarEvent`. Sharing via `CalendarAccountUser` (viewer/editor/manager, mirrors `EmailAccountUser`). `CalendarEvent.accessible_to(user)` is the permission gate (mirrors `EmailMessage`). Encrypted refresh token; provider enum `google/zoho`.
- **Sync** (`CalendarScanJob`, two-tier in `config/recurring.yml`: `calendar_scan` every minute incremental + `calendar_scan_full` every 15 min): slot-lock like `EmailScanJob`; per-calendar incremental pull via stored sync token, full pull (`singleEvents=true`, capped window) on the full sweep / when a calendar has no events yet / on HTTP-410 (`Calendars::FullResyncJob`, jittered + rate-limited). Loop-avoidance via `provider_etag` + `outbound_pending`; cancellations tombstone (status `cancelled`). Only the **primary** calendar auto-enables; others are user toggles (Settings → Calendars).
- **Outbound (two-way)**: `Calendars::EventWriter` + `Calendars::EventWriteJob` push create/update/delete/RSVP with an `If-Match` etag guard (412 → re-fetch + retry, last-write-wins).
- **Provider clients**: `Google::CalendarClient` (full), `Zoho::CalendarClient` (⚠️ written but **unverified** against a live Zoho grant). Both normalize to one common event hash; connection rebuilt per request for the fresh cached token (mirrors the mail clients).
- **Views**: `/calendar` with `?view=agenda|day|week|month` (`CalendarController`) — `Campbooks::Calendar::{AgendaList,DayGrid,WeekGrid,MonthGrid}`. Day is a time-grid (hour rows, overlap columns, now-line). Event CRUD via `CalendarEventsController` (`/calendar_events/*`). Filled chips auto-pick readable text via `Campbooks::Base#contrast_on` (white-on-pale-calendar-color would otherwise fail WCAG).
- **Event color**: events carry an optional per-event `color` (hex); `CalendarEvent#display_color` falls back to the calendar color when unset, so all render sites read `event.display_color`. The picker (`Campbooks::ColorSwatchPicker`) and two-way sync use the **fixed Google event palette** — `Calendars::EventColors` maps hex↔Google `colorId` (1–11), Zoho passes the hex through. Color flows both ways via the normal sync (`normalize_event`/`build_payload` + `EventWriter#attrs_for_provider`); a blank color clears back to the calendar default.
- **Email → event**: `create_calendar_event` in the `EmailActions` registry (surfaces single/palette/scout_suggest/workflow) → `Tools::CreateCalendarEvent` + `Ai::EventExtractor` (heuristic title/time/location), links `source_email_message`. Wired across inbox, Cmd+K, Scout, and workflows.
- **Feed**: `Feed::Sources::CalendarEvent` + `Campbooks::Feed::CalendarEventCard` surface imminent meetings.
- **⚠️ Provider setup**: enable the **Google Calendar API** on the `GOOGLE_CLIENT_ID` project + add `calendar`/`calendar.events` to the consent screen; add `ZohoCalendar.event.ALL`/`ZohoCalendar.calendar.ALL` to the Zoho app. Without the API enabled, calendar list returns `403 accessNotConfigured`. **Google Tasks are not calendar events** (separate API) — not synced.
- **Push**: `CalendarWebhooksController#google_receive` (public; verifies the per-channel token → enqueues an incremental sync) + `Calendars::WebhookRenewalJob` (daily-ish; registers/renews `events.watch` channels, stored in `calendar_webhook_channels`). **Prod-gated**: a no-op unless a public callback host is set (`APP_HOST` or the mailer host) and `DISABLE_CALENDAR_PUSH` is unset; dev relies on the minute poll. Google only (Zoho polls).

## Workflow engine

Workspace-scoped automations (`Workflow` → ordered `WorkflowStep`s) that run when a trigger fires. Each run is recorded as a `WorkflowExecution` with one `WorkflowExecutionStep` per step (input/output captured for debugging). UI: `/workflows` (list), `/workflows/:id/edit` (builder), `/workflows/:id/executions` (run history).

- **Triggers** (`Workflow#trigger_type`): `email_received` (fired by `WorkflowTriggerJob` from `EmailProcessJob`) and `webhook` (an external service POSTs to `/webhooks/:token`, handled by the public, unauthenticated `WebhooksController` → `WorkflowWebhookJob`). The token is minted automatically and rotatable via `regenerate_webhook`.
- **Actions** (`WorkflowStep#action_type`): `send_email`, `http_request` (generic outbound call), `slack_message`, `discord_message` (incoming-webhook wrappers), and `custom_action` (calls a saved `Connection` — see below). All HTTP-backed actions share `Workflows::HttpClient` (Faraday, timeouts, normalized result) guarded by `Workflows::UrlGuard` (blocks loopback/private/link-local/metadata hosts; local hosts allowed only in development).
- **Conditions** evaluate against the trigger via `Workflows::ConditionEvaluator` (the email `document_type` check, or a generic Liquid field path like `payload.status` for webhooks). A failing condition halts the run.
- **Templating**: every step field is rendered with Liquid by `Workflows::LiquidRenderer` against a `Workflows::TriggerContext` (`EmailContext` exposes `email`/`documents`; `WebhookContext` exposes `payload`/`headers`/`query`). Variables are lenient (missing → empty), filters are strict.
- **Builder UI**: steps are added via a Zapier-style picker — every "+" connector opens the shared `Campbooks::StepPicker` modal (search + cards), driven by the `step-picker` Stimulus controller. The action type can also be switched after the fact via the in-card dropdown in `Campbooks::WorkflowStepForm`.
- **Action registry**: action types are defined once in `Workflows::ActionRegistry` (`app/services/workflows/action_registry.rb`). `WorkflowStep::ACTION_TYPES`/`ACTION_LABELS`/`HTTP_ACTION_TYPES`, the `Campbooks::StepPicker` catalog, the action `<select>` in `Campbooks::WorkflowStepForm`, and `WorkflowsController#workflow_params` all derive from it.
- **Adding an action type**: add one `Definition` to `ActionRegistry` (`key`, `label`, `icon`, `description`, `config_schema`, and either a `build:` request-builder or a `run:` runner naming a `Workflows::Executor` method). Add that `build_*`/`execute_*` method only if it needs new logic, and a panel in `Campbooks::WorkflowStepForm` only if it needs custom fields. Full design in `docs/workflow-actions.md`.
- **Custom Action + Connections**: `custom_action` calls a saved `Connection` (workspace-scoped base URL + encrypted auth, managed at Settings → Integrations → Connections via `Settings::Integrations::ConnectionsController`). The executor resolves the connection server-side and merges its auth header, so secrets never live in a step's Liquid; the resolved URL still passes through `HttpClient`/`UrlGuard`.

## Public REST API

Customer-facing REST API for programmatic access to a workspace's own data. **Doorkeeper** provides OAuth 2.0 with the **client_credentials** grant only. Full reference: [`docs/api.md`](docs/api.md). Built in three phases, **all live**: **P1** emails + documents; **P2** contacts, tags (email-only), document types, document writes (update/approve/reject/reclassify); **P3** workflows (list, executions read, authenticated `:trigger` for webhook workflows via `WorkflowWebhookJob`) + Scout chat (async: `POST` a message → 202 → poll `GET …/messages?after_message_id=N` for the AI reply; `AgentChatReplyJob` re-derives its own `Current` from the thread).

- **Auth bridge (the crux)**: client_credentials tokens have no resource owner. Each `Doorkeeper::Application` carries `workspace_id` + `created_by_id` (added in a migration; associations decorated in `config/initializers/doorkeeper_application_extensions.rb`). `Api::V1::BaseController#establish_acting_identity!` resolves them into `Current.workspace` + `Current.acting_user`, so the existing gates (`EmailMessage.accessible_to(Current.user)`, `Current.workspace.<assoc>`, `sendable_email_accounts`) apply **unchanged**. Fails closed (401 `client_revoked`) if the workspace/user is gone.
- **Token endpoint**: `POST /api/oauth/token` (+ `/api/oauth/revoke`), mounted at `/api/oauth` to avoid the inbound provider callbacks in `namespace :oauth`. Introspection + the authorize/applications controllers are disabled (`config/initializers/doorkeeper.rb`).
- **Base controller** (`app/controllers/api/v1/base_controller.rb`, inherits `ActionController::Base`): bearer auth, per-client `rate_limit`, JSON envelope (`{ data, meta }` / `{ error: { code, message } }`), and the **404-not-403** leak rule. Per-action scopes via `before_action -> { doorkeeper_authorize! :"emails:send" }`.
- **Scopes**: catalog + descriptions in `Api::Scopes` (`app/models/api/scopes.rb`), mirrored by `optional_scopes` in the initializer (a spec guards the two against drift). Secrets BCrypt-hashed, tokens SHA256-hashed, **secret shown once** at create/rotate.
- **Serializers**: plain POROs in `app/serializers/api/v1/` (no serializer gem). **Send logic** is shared with the web composer via `Emails::Sender` (`app/services/emails/sender.rb`).
- **Manage clients**: Settings → API access (`Settings::ApiClientsController`, workspace-scoped). **Adding a resource**: new `Api::V1::*Controller` < `BaseController` + a serializer + routes under `namespace :api { namespace :v1 }` + new scope(s) in both `Api::Scopes` and the initializer.

## Key env vars

- `ZOHO_CLIENT_ID`, `ZOHO_CLIENT_SECRET` — app-level OAuth credentials
- `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET` — Gmail "Sign in with Google" + mailbox connect
- `GOOGLE_DRIVE_CLIENT_ID`, `GOOGLE_DRIVE_CLIENT_SECRET` — separate Google project for the Drive integration (`Oauth::GoogleController`). Scope is the **full `drive`** scope (browse/pick any folder for the interactive "Send to Drive" flow), which is a Google **restricted** scope — the consent screen must pass Google verification before prod, and accounts connected under the old `drive.file` scope must reconnect (see `GoogleDriveAccount#full_access?`).
- `NOTION_CLIENT_ID`, `NOTION_CLIENT_SECRET` — Notion **public integration** OAuth (`Oauth::NotionController`), enabling multi-workspace connect + the interactive "Send to Notion" / workflow / Scout actions. When unset (e.g. self-hosted), Settings → Integrations → Notion falls back to a manually-pasted internal integration token. Register the integration at notion.so/my-integrations (type: Public) with the redirect URI below and Read/Insert content capabilities.
- `MICROSOFT_CLIENT_ID`, `MICROSOFT_CLIENT_SECRET` — Microsoft 365 "Sign in with Microsoft" + mailbox connect. Register an app in Entra (`https://entra.microsoft.com` → App registrations), supported account types "any organizational directory (multitenant)" (the OAuth client uses the `/organizations/` endpoint — work/school accounts only, no personal MSAs), with a client secret. Both `session#microsoft` (sign-in) and `EmailAccountsController#create` (connect) read these, so they're required for either flow.
- `ACTIVE_RECORD_PRIMARY_KEY`, `ACTIVE_RECORD_DETERMINISTIC_KEY`, `ACTIVE_RECORD_KEY_DERIVATION_SALT` — encryption keys for `EmailAccount#refresh_token`
- OAuth callback URLs must be whitelisted in each provider's console: `/oauth/zoho/callback` (Zoho), `/oauth/gmail/callback` (Google), `/oauth/microsoft/callback` (Microsoft/Entra), `/oauth/google/callback` (Google Drive), `/oauth/notion/callback` (Notion) — for both `http://localhost:3000` (dev) and `https://app.campbooks.not-a-camp.com` (prod)

## Integrations: Google Drive & Notion (interactive)

Beyond the background document-sync paths (auto-archive to a Drive folder via `GoogleDriveConfig`; map document fields into a Notion DB via `NotionDatabaseMapping`), there are **interactive, on-demand actions**:

- **Shared layer** (`app/services/integrations/`): `FileSource` resolves the context's files (a Document's `original_file`, or an EmailMessage's attachments) into uniform descriptors; `Drive::{FolderCreator,FileUploader}` and `Notion::{PageCreator,DatabaseItemCreator}` wrap the API clients. `GoogleDrive::Client` gained `list_folders`/`get_folder`; `Notion::Client` gained page-parent `create_page_under` + `search`/`list_pages`; `Notion::FileUploader` runs the Notion File Upload API (≤20 MB single-part) to attach files to "files" properties; `Notion::PropertyBuilder` turns `{prop => {type, value}}` into a Notion properties payload.
- **Documents** (`Documents::{DriveExportsController,NotionExportsController}` + `documents/show` buttons): "Send to Drive" (browse/create folder → upload) and "Send to Notion" (pick workspace → database with a schema-driven `Campbooks::Notion::DatabaseForm` showing one input per field, files → a files property; or a subpage under a chosen page).
- **Workflows** (`Workflows::ActionRegistry`): `google_drive_create_folder`, `google_drive_upload`, `notion_create_page`, `notion_create_database_item` (file source = the triggering email's attachments; Notion DB-item properties supplied as a Liquid JSON object resolved against the live DB schema at run time).
- **Scout & Cmd+K** (`EmailActions`): `upload_attachments_to_drive` (surfaces `palette`, `scout_suggest`) uploads an email's attachments to Drive.
- **Logos**: `Campbooks::BrandLogo` renders the Google Drive / Notion / Zoho marks on the integration cards, settings pages, and connect buttons.

## Internationalization (i18n)

The app ships in **English (source), Portuguese (pt-PT), Spanish, and French**, all at full key parity (enforced by i18n-tasks).

- **Config**: `available_locales`/`default_locale`/`fallbacks` in `config/application.rb`; locale files are split by domain under `config/locales/<locale>/*.yml` (loaded via a `**` glob). The test env sets `raise_on_missing_translations`.
- **Per-request locale**: `ApplicationController` `around_action :switch_locale` resolves `params[:locale]` → `current_user.locale` → `Accept-Language` → default. Users pick a language in **Settings → Account** (`User#locale` column + `#language` action).
- **Views/controllers**: lazy keys `t(".key")`. **Phlex components**: `Campbooks::Base#t`/`#l` scope a `.key` to `components.<snake_class>`; use an absolute key (`t("shared.actions.x")`) for shared strings. **Mailers**: subjects via `t(".subject")`, rendered in the recipient's locale via `with_recipient_locale`.
- **Enums**: `human_enum(Model, :attr, value)` → `activerecord.attributes.<model>.<attr_plural>.<value>`. **Dates/numbers/currency**: `l(value, format: :name)` with named formats in `config/locales/<locale>/formats.yml` (defined under **both** `time.formats` and `date.formats` so they resolve for Time and Date values); currency auto-localizes via Money + rails-i18n.
- **Tooling**: `bundle exec i18n-tasks missing|unused|health` (`config/i18n-tasks.yml`; `app/components` is excluded — Phlex's custom scope — and method-name lazy-key false positives are suppressed). Extraction conventions: `docs/i18n-extraction-guide.md`.
- ⚠️ **Gotcha**: never put a lazy `t(".")` inside `render layout: "x" do … end` — the block resolves keys against the *layout's* virtual path, not the page's. Use `content_for :sidebar` inline instead (see `settings/*` views).

## Deployment & secrets

Deployment, secrets, and ops are private — see **"🔒 PUBLIC REPO — KEEP IT CLEAN"**
at the top of this file and the private `campbooks-cloud` repo. Self-hosting
instructions are in [`docs/self-hosting.md`](docs/self-hosting.md).
