# REST API ↔ UI parity roadmap

**Goal.** Bring the public REST API (`Api::V1`) to full parity with what the web
UI can do, so a first-party React client can be built entirely on the API and
eventually replace the server-rendered UI.

**Method.** Every web action funnels through a framework-agnostic service layer
(`EmailActions`, `Emails::Sender`, `Tools::*`, `Reminders::Confirm`, …). The API
must **call that same layer**, never reimplement it — the web controllers stay
thin Turbo wrappers, the API controllers stay thin JSON wrappers, and both share
one place for permission/entitlement checks. Where a web action's logic still
lives inline in its controller, **extract it into a service first**, then have
both surfaces call it.

**Conventions** (already established — match them):
- Controllers under `app/controllers/api/v1/`, inherit `BaseController`
  (bearer auth, `Current.workspace`/`Current.acting_user` bridge, JSON envelope,
  404-not-403 leak rule).
- Per-action scope gate: `before_action -> { doorkeeper_authorize! :"x:write" }`,
  or `doorkeeper_authorize!(scope)` inside the body when the scope is dynamic.
- Serializers are plain POROs in `app/serializers/api/v1/`.
- New scopes go in **both** `app/models/api/scopes.rb` (`CATALOG`) and
  `config/initializers/doorkeeper.rb` (`optional_scopes`) — a spec guards the two
  against drift.
- Every endpoint gets a `spec/requests/api/v1/*_spec.rb` and a `docs/api.md`
  section. Update `CHANGELOG.md` (`[Unreleased] → Added`).

Each chunk below is one PR (`feat: …`), production-ready and independently
mergeable. Order is roughly by leverage for a React inbox.

---

## Tier 1 — the inbox core (a React client is unusable without these)

### ✅ Chunk 1 — single-message email triage actions
`POST /api/v1/emails/:id/actions/:name` dispatched through `EmailActions.run`.
Actions: archive/unarchive, trash, snooze/unsnooze, pin/unpin, forward_email,
star/unstar/block/unblock/allow sender. Scopes: emails:write / emails:send /
contacts:write. **Shipped.** No new services needed — the registry already
existed.

### ✅ Chunk 2 — bulk email actions
`POST /api/v1/emails/bulk/:name` mirroring `EmailMessages::BulkController`:
archive, unarchive, mark_read/unread, move_to_folder, tag, delete,
snooze/unsnooze. Group-expansion + thread-expansion + tool dispatch + inbox
broadcast were **extracted into `Emails::BulkActions`**; the web controller and
the API controller now both call it (web behaviour proven unchanged by
`spec/requests/email_messages/bulk_group_spec.rb`). UI-only tools (`forward`
compose, `process_ai`, `scout_chat`) intentionally not exposed. **Shipped.**

### ◑ Chunk 3 — drafts & compose
`resources :drafts` (DraftEmail CRUD, `dismissed` as a settable boolean) shipped
behind new `drafts:read`/`drafts:write` scopes. Sending reuses the existing
`/emails` + `/emails/:id/reply` endpoints (`Emails::Sender`). **Still to do
(chunk 3b):** attachment + inline-image upload endpoints (ActiveStorage direct
upload) so an API client can populate `attachments` signed ids, and an optional
`POST /drafts/:id/send` convenience.

### ◑ Chunk 4 — threads read + follow/unfollow
`GET /api/v1/threads` (list) + `GET /api/v1/threads/:id` (thread with ordered
messages + `following`) + `POST`/`DELETE /threads/:id/follow` (`ThreadFollow`).
Scopes: `emails:read` / `emails:write`. **Shipped.** **Deliberately deferred:**
custom-folder create/rename/delete (`MailFolder`) — these fire provider-label
side effects (`MailFolders::Provisioner`) and were intentionally excluded from
the API's existing `folders` resource; revisit as its own chunk with a clear
decision on whether the API should mutate provider folders.

### ☐ Chunk 5 — email accounts resource
`resources :email_accounts` (index/show + status). The `email_accounts:read`
scope already exists but has **no controller** — pure documented-but-missing gap.
Read-only first; connect/reauth flows stay OAuth-interactive.

### ☐ Chunk 6 — close the write-scope gaps
Endpoints the granted scopes already imply but don't back: `tags:write` create
(workspace tag, `POST /api/v1/tags`), `document_types:write` create, `folders`
create. Small, removes "scope grants power with no endpoint" drift.

---

## Tier 2 — feature-complete surfaces the UI has and the API half-has

### ☐ Chunk 7 — calendar completeness
event_types (calendar "tags") CRUD, calendar/account sync-toggle + color,
per-user visibility, `reschedule`, `.ics` import. Core event CRUD + rsvp already
shipped.

### ☐ Chunk 8 — contacts & organizations
Contact `analyze`, dedup/`resolve_duplicate`, `lookup`/`search`. **Organizations
have no API at all** — add `resources :organizations` (index/show/update +
emails/documents sub-lists).

### ☐ Chunk 9 — files & filing
The unified file manager: files/folders tree, uploads (create/destroy/analyze),
folder shares, public links. Distinct from the existing `documents` resource.

### ☐ Chunk 10 — notifications
`resources :notifications` (index/show + mark_read/archive/mark_all_read) +
preferences. A React shell needs the notification bell.

### ☐ Chunk 11 — reminders create + tasks completeness
Reminders are read+state today; add create if the UI allows manual creation.
Tasks: add `destroy`/`archive`/`unarchive`, comments, email/document link
endpoints.

### ☐ Chunk 12 — search & feed
Global search (`/api/v1/search`) backing Cmd+K, and the home feed items
(`act`/`dismiss`/`seen`/`undo`). Both are read-heavy surfaces a React home needs.

### ☐ Chunk 13 — remaining Tier 2 (as needed)
Signatures, email rules, tag groups, digests, pipelines, Notion/Drive export
actions, workflow create/update. Sequence by what the React client reaches for.

---

## Tier 3 — account/workspace management (deferred)

Settings (account, security/2FA, members, invitations, integrations, API
clients), onboarding, admin. A React client can defer these behind the
server-rendered settings pages longest, so they come last.

---

## Cross-cutting, do alongside

- **Scope hygiene:** keep `Api::Scopes::CATALOG` ↔ doorkeeper `optional_scopes`
  in sync (guarded by `spec/models/api/scopes_spec.rb`).
- **MCP mirror:** the `/api/mcp` JSON-RPC surface mirrors the REST resources with
  the same scopes. When a REST resource gains actions, decide whether the MCP
  tool catalog should mirror them (many already do).
- **OpenAPI:** if `docs/` carries an OpenAPI spec, extend it per chunk.
- **404-not-403 leak rule:** every new endpoint scopes its lookup through
  `accessible_to` / `Current.workspace.<assoc>` and 404s on cross-tenant IDs.
