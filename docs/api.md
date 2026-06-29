# Campbooks Public REST API

The public API lets customers reach their own Campbooks data — email,
documents, contacts, tags, document types, Scout chat, calendar events,
scheduled emails, email and document templates, reminders, and folders —
from their own apps and scripts. It is authenticated with **OAuth 2.0 client
credentials** via [Doorkeeper](https://github.com/doorkeeper-gem/doorkeeper).

> **Machine-readable spec:** a full [OpenAPI 3.0 description](../openapi.yaml)
> lives at [`openapi.yaml`](../openapi.yaml) in the repo root — use it to
> generate clients or import into Postman/Insomnia. The hosted, browsable
> reference renders that same file.

> Replace `https://<your-campbooks-host>` below with your deployment's host
> (e.g. the value of `APP_HOST`). All endpoints are served over HTTPS in
> production.

## Quickstart

```bash
# 1. Create a client in Settings → API access and copy its ID + secret.
# 2. Exchange them for a bearer token (valid 2 hours). Always pass `scope`.
TOKEN=$(curl -s -X POST https://<your-campbooks-host>/api/oauth/token \
  -d grant_type=client_credentials \
  -d client_id=YOUR_CLIENT_ID \
  -d client_secret=YOUR_CLIENT_SECRET \
  -d "scope=emails:read" | jq -r .access_token)

# 3. Call the API with the token.
curl https://<your-campbooks-host>/api/v1/emails \
  -H "Authorization: Bearer $TOKEN"
```

## Authentication

The API uses the **client-credentials** grant: a credential acts as the user
who created it, inside that user's workspace. There is no per-end-user
authorization step.

1. In the app, go to **Settings → API access** and create a client. Choose the
   scopes it needs. You'll see the **client ID** and, **once**, the **client
   secret** — store the secret somewhere safe (it's hashed at rest and can't be
   shown again; you can regenerate it later).
2. Exchange the credentials for a short-lived bearer token (valid 2 hours):

```bash
curl -X POST https://<your-campbooks-host>/api/oauth/token \
  -d grant_type=client_credentials \
  -d client_id=YOUR_CLIENT_ID \
  -d client_secret=YOUR_CLIENT_SECRET \
  -d "scope=emails:read documents:read"
```

Response:

```json
{
  "access_token": "…",
  "token_type": "Bearer",
  "expires_in": 7200,
  "scope": "emails:read documents:read",
  "created_at": 1750000000
}
```

3. Call the API with the token:

```bash
curl https://<your-campbooks-host>/api/v1/emails \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN"
```

> ⚠️ Always pass a `scope` parameter when requesting a token. A token requested
> with no scope can't call anything and every endpoint will return
> `insufficient_scope`.

You can revoke a token via `POST /api/oauth/revoke`, or revoke **all** of a
client's tokens (and rotate its secret) from Settings → API access.

### Browser sign-in (the CLI)

For interactive use there's also the **[Campbooks CLI](cli.md)** — a single
binary that wraps this API. Instead of pasting a client ID/secret, run
`campbooks login`: it opens your browser, you sign in with your normal Campbooks
session, approve the "Campbooks CLI" client, and the CLI receives a token scoped
to **you** in your workspace. Under the hood that's the OAuth 2.0
**authorization-code + PKCE** grant against the first-party public client
`campbooks-cli`, with a loopback redirect (`http://127.0.0.1:<port>/callback`):

1. `GET /api/oauth/authorize?response_type=code&client_id=campbooks-cli&redirect_uri=…&scope=…&code_challenge=…&code_challenge_method=S256`
   — renders the consent screen (signing the user in first if needed).
2. On approval the browser is redirected to the loopback URL with `?code=…`.
3. `POST /api/oauth/token` with `grant_type=authorization_code`, the `code`, and
   the matching `code_verifier` → an access token **plus a refresh token**, so
   the CLI stays signed in without re-opening the browser.

This grant is reserved for the first-party CLI (PKCE is mandatory and there's no
client secret). Build your own integrations on the client-credentials grant
above.

## Scopes

| Scope | Grants |
|-------|--------|
| `emails:read` | List/read email messages, threads, folders (for accounts the credential's user can read) |
| `emails:write` | Mark emails read/unread |
| `emails:send` | Send and reply to email (from accounts the user can send from) |
| `documents:read` | List/read documents and download files |
| `documents:write` | Upload, update, approve, reject, and reclassify documents |
| `contacts:read` | List/read contacts |
| `contacts:write` | Update contacts and change their state (star/block/allow) |
| `tags:read` | List the workspace's tags |
| `tags:write` | Add/remove tags on emails |
| `document_types:read` | List the workspace's document types |
| `scout:read` | Read Scout chat threads and messages |
| `scout:write` | Create Scout threads and send messages |
| `scheduled_emails:read` | List and read scheduled emails |
| `scheduled_emails:write` | Schedule, update, and cancel emails |
| `calendar:read` | Read calendar events |
| `calendar:write` | Create, update, RSVP, and delete calendar events |
| `reminders:read` | Read AI reminders |
| `reminders:write` | Confirm, dismiss, and snooze reminders |
| `tasks:read` | List and read tasks |
| `tasks:write` | Create, update, and complete tasks |
| `folders:read` | List folders and their contents |
| `folders:write` | File and unfile documents in folders |

<!-- The `workflows:read` / `workflows:trigger` scopes are omitted while the Workflows feature is disabled by default (ENABLE_WORKFLOWS). Restore both rows above when it ships. -->

Scopes are a ceiling, not a grant of new power: a request must satisfy **both**
the token's scope **and** the acting user's own permissions (e.g. `emails:send`
still requires that the user may send from the chosen account).

## Conventions

- **Base path:** `/api/v1`
- **Format:** JSON. Collections are `{ "data": [ … ], "meta": { … } }`; single
  resources are `{ "data": { … } }`.
- **Pagination:** `?page=` and `?per_page=` (default 25, max 100). `meta`
  carries `page`, `per_page`, `total`, `total_pages`. Two small reference
  collections — **document types** and **Scout messages** — are returned
  unpaginated and carry **no `meta`**.
- **Rate limit:** 600 requests/minute per client (HTTP 429 when exceeded).
- **Errors:** `{ "error": { "code": "…", "message": "…" } }`, with validation
  details under `error.details` where relevant.

### Error codes

Every error response is `{ "error": { "code": "…", "message": "…" } }`.

| Status | `error.code` | When |
|--------|--------------|------|
| 400 | `missing_parameter` | A required parameter is absent |
| 401 | `invalid_token` | Missing/invalid/expired/revoked token |
| 401 | `client_revoked` | The client's workspace/user no longer exists or matches |
| 401 | `account_pending_deletion` | The acting user's account is scheduled for deletion |
| 403 | `insufficient_scope` | Token lacks the scope (or has none) for the action |
| 403 | `no_sendable_account` | `POST /emails` or `/reply`: the user can't send from that account |
| 404 | `not_found` | The resource doesn't exist **or isn't visible to you** |
| 404 | `no_file` | `GET /documents/:id/file`: the document has no attached file |
| 422 | `validation_failed` | The payload was rejected (`error.details`) |
| 422 | `invalid_state` | `POST /contacts/:id/state`: `state` wasn't a valid value |
| 429 | `rate_limit_exceeded` | Too many requests |
| 403 | `entitlement_required` | The workspace plan does not include this feature |
| 403 | `calendar_not_writable` | `POST /calendar_events`: the calendar is not writable or does not exist |
| 403 | `event_not_writable` | Mutating a calendar event whose calendar is not writable by the user |
| 422 | `invalid_rsvp_status` | `POST /calendar_events/:id/rsvp`: unrecognized `rsvp_status` value |
| 422 | `confirm_failed` | `POST /reminders/:id/confirm`: reminder confirmation failed |
| 503 | `ai_provider_unconfigured` | Scout: the workspace has no AI provider for chat |

> 404 (not 403) is returned for resources you can't see, so the API never
> reveals the existence of another workspace's data.

## Emails

### `GET /api/v1/emails` — list (scope `emails:read`)

Filters: `account_ids[]`, `unread` (bool), `has_attachment` (bool), `category`,
`priority` (`low`/`medium`/`high`), `q` (matches subject/sender),
`received_after`, `received_before` (ISO 8601), plus `page`/`per_page`.

```json
{
  "data": [
    {
      "id": 42, "subject": "Invoice", "from": "billing@acme.com",
      "to": "me@example.com", "cc": null, "read": false,
      "has_attachment": true, "priority": "high", "category": "finance",
      "summary": "…", "pinned": false, "received_at": "2026-06-23T09:00:00Z",
      "thread_id": 7, "account_id": 3, "tags": ["receipts"]
    }
  ],
  "meta": { "page": 1, "per_page": 25, "total": 1, "total_pages": 1 }
}
```

### `GET /api/v1/emails/:id` — show (scope `emails:read`)

Same fields plus `bcc` and the full `body` (HTML).

### `POST /api/v1/emails/:id/mark_read` · `…/mark_unread` (scope `emails:write`)

Marks the message read/unread. `mark_read` also syncs the flag to the provider
mailbox. Returns the updated email (`{ "data": { … } }`).

### `POST /api/v1/emails` — send (scope `emails:send`)

Body: `email_account_id` (required), `to_address` (required), `subject`, `body`,
`cc_address`, `bcc_address`. Returns `201` with
`{ "data": { "id": …, "provider_message_id": "…" } }`. Returns `403`
`no_sendable_account` if the acting user can't send from that account.

### `POST /api/v1/emails/:id/reply` — reply (scope `emails:send`)

Body: `body` (required), optional `to_address` (defaults to the original
sender), `cc_address`, `bcc_address`, `email_account_id` (defaults to the source
message's account). Threads automatically.

## Documents

### `GET /api/v1/documents` — list (scope `documents:read`)

Filters: `type` (document type id), `review_status` (`pending`/`approved`/
`rejected`), `ai_status` (`pending`/`processing`/`completed`/`failed`), plus
`page`/`per_page`. Documents are workspace-wide (every member sees them).

```json
{
  "data": [
    {
      "id": 88, "title": "ACME invoice #INV-1", "document_type_id": 3,
      "document_type": { "id": 3, "name": "Invoice", "category": "finance" },
      "ai_status": "completed", "review_status": "pending",
      "source": "email", "starred": false, "document_date": "2026-06-20",
      "vendor_name": "ACME", "client_name": null, "invoice_number": "INV-1",
      "amount_cents": 12900, "currency": "EUR", "description": "…",
      "canonical_filename": "2026-06-20_acme_inv-1.pdf",
      "created_at": "2026-06-23T09:01:00Z"
    }
  ],
  "meta": { "page": 1, "per_page": 25, "total": 1, "total_pages": 1 }
}
```

(`document_type` is the full nested type record, or `null` when unclassified.)

### `GET /api/v1/documents/:id` — show (scope `documents:read`)

Adds `file` (the attached file's `filename`, `content_type`, `byte_size`, and a
`download_path`) and the raw AI `extraction` data.

### `GET /api/v1/documents/:id/file` — download (scope `documents:read`)

Streams the original uploaded file (send the same bearer token). `404` `no_file`
if the document has no attached file.

### `POST /api/v1/documents` — upload (scope `documents:write`)

Multipart `files[]` (one or more). AI classification/extraction runs
asynchronously, so the response is **`202 Accepted`** and each document starts
with `ai_status: "pending"`. The body is `{ "data": [ … ] }` (one entry per
uploaded file).

```bash
curl -X POST https://<your-campbooks-host>/api/v1/documents \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN" \
  -F "files[]=@invoice.pdf"
```

### `PATCH /api/v1/documents/:id` — update fields (scope `documents:write`)

Send any subset of the editable extracted fields; only the fields you send
change. Editing fields does **not** change review state (use approve/reclassify
for that).

Editable fields: `document_type_id`, `vendor_name`, `vendor_nif`,
`document_date`, `due_date`, `invoice_number`, `amount_cents`, `currency`,
`buyer_nif`, `tax_amount_cents`, `tax_rate`, `description`, `expense_category`,
`company_vat_present`, `client_name`, `client_nif`, `bank_name`,
`account_number`, `period_start`, `period_end`, `opening_balance_cents`,
`closing_balance_cents`, `receipt_number`, `payment_method`, and `metadata` (an
object).

### `POST /api/v1/documents/:id/approve` · `…/reject` (scope `documents:write`)

Approve or reject a document under review; approve records the acting user as the reviewer.

### `POST /api/v1/documents/:id/reclassify` — change type (scope `documents:write`)

Body: `document_type_id` (required). Reclassifying also signs the document off (it becomes `approved`).

## Contacts

Contacts are created automatically from email sync — there is **no create endpoint**. They are workspace-wide.

### `GET /api/v1/contacts` — list (scope `contacts:read`)

Filters: `list_status` (`neutral`/`allowed`/`blocked`), `starred` (bool), `q` (matches name/email), plus `page`/`per_page`.

```json
{
  "data": [
    {
      "id": 5, "email": "jane@acme.com", "name": "Jane Doe",
      "organization": "ACME", "relationship_type": "client",
      "list_status": "neutral", "starred": true, "email_count": 12,
      "last_email_at": "2026-06-22T18:00:00Z",
      "context_summary": "…", "person_id": 9
    }
  ],
  "meta": { "page": 1, "per_page": 25, "total": 1, "total_pages": 1 }
}
```

### `GET /api/v1/contacts/:id` — show (scope `contacts:read`)

### `PATCH /api/v1/contacts/:id` — update (scope `contacts:write`)

Body: `name`, `relationship_type` (one of
`self`/`client`/`vendor`/`partner`/`service_provider`/`colleague`/`personal`/`unknown`).
Only the fields you send change. (`organization` and the other contact fields
are read-only here.)

### `POST /api/v1/contacts/:id/state` — change state (scope `contacts:write`)

Body: `state` — one of `star`, `unstar`, `allow`, `block`, `unblock`. Blocking
also archives the contact's inbox messages. An unrecognized value returns `422`
`invalid_state`.

## Tags

Tags apply to **emails only** (documents are organized by document type instead).

### `GET /api/v1/tags` — list (scope `tags:read`)

```json
{
  "data": [
    {
      "id": 2, "name": "receipts", "color": "#16a34a",
      "group_name": "Finance", "source": "user", "email_account_id": null
    }
  ],
  "meta": { "page": 1, "per_page": 25, "total": 1, "total_pages": 1 }
}
```

(`email_account_id` is set when a tag is scoped to a single account; `null` for
workspace-wide tags.)

### `POST /api/v1/emails/:id/tags` — add a tag (scope `tags:write`)

Body: `tag_id` **or** `name` (must be an existing workspace tag — tags aren't created here). Returns `201` with the attached tag.

### `DELETE /api/v1/emails/:id/tags/:tag_id` — remove a tag (scope `tags:write`)

Returns `204 No Content`.

## Document types

### `GET /api/v1/document_types` — list (scope `document_types:read`)

Returns the workspace's document types (`id`, `name`, `color`, `category`,
`auto_star`, `extraction_schema`) — use a type's `id` with document
upload/update and reclassify. **Unpaginated** (no `meta`).

```json
{
  "data": [
    {
      "id": 3, "name": "Invoice", "color": "#2563eb", "category": "finance",
      "auto_star": false, "extraction_schema": { "…": "…" }
    }
  ]
}
```

## Scout

Scout is the in-app AI assistant. The API is **asynchronous**: you post a user
message, get a `202` immediately, and the AI reply is generated by a background
job. You then **poll** the messages endpoint for the reply. Threads are scoped to
the credential's acting user.

### `GET /api/v1/scout/threads` — list threads (scope `scout:read`)

Returns the acting user's chat threads (`id`, `title`, `purpose`, timestamps),
newest-first. Paginated.

### `POST /api/v1/scout/threads` — create a thread (scope `scout:write`)

Body: optional `title` (defaults to "New chat"). Returns `201` with the thread.

### `GET /api/v1/scout/threads/:id/messages` — read / poll (scope `scout:read`)

Returns the thread's messages in chronological order (**unpaginated** — no
`meta`). Pass `?after_message_id=N` to fetch only messages created **after**
message `N` — the poll loop for the async reply. Each message carries `id`,
`thread_id`, `author_type` (`user`/`ai`), `content`, `reply_status`
(`pending`/`processing`/`replied`/`failed`), `suggested_actions`, `prompts`, and
`created_at`.

### `POST /api/v1/scout/threads/:id/messages` — send a message (scope `scout:write`)

Body: `content` (required). Returns **`202 Accepted`** with the created user
message. The AI reply lands later as a new message with `author_type: "ai"` and
`reply_status: "replied"` — poll
`GET …/messages?after_message_id=<the returned id>` until it appears.

```bash
# 1. Post the message → 202 with { "data": { "id": 100, "author_type": "user", … } }
curl -X POST https://<your-campbooks-host>/api/v1/scout/threads/7/messages \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN" \
  -d "content=What needs my attention today?"

# 2. Poll for the reply
curl "https://<your-campbooks-host>/api/v1/scout/threads/7/messages?after_message_id=100" \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN"
```

> Sending a message returns `503 ai_provider_unconfigured` if the workspace has
> no AI provider set up for chat.

## Scheduled Emails

Scheduled emails are workspace-scoped. Writes (create/update/cancel) require the
`:email_scheduling` entitlement; the workspace plan must include email scheduling
or the API returns `403 entitlement_required`. Creating or updating to an account
the acting user cannot send from returns `403 no_sendable_account`.

### `GET /api/v1/scheduled_emails` — list (scope `scheduled_emails:read`)

Filter by `status` (`pending`/`sent`/`cancelled`/`failed`), plus `page`/`per_page`.
Ordered by soonest next occurrence.

```json
{
  "data": [
    {
      "id": 11, "to": "client@acme.com", "cc": null, "bcc": null,
      "subject": "Monthly report", "status": "pending",
      "recurring": true, "rrule": "FREQ=MONTHLY;BYMONTHDAY=1",
      "scheduled_at": "2026-07-01T09:00:00Z",
      "next_occurrence_at": "2026-07-01T09:00:00Z",
      "last_sent_at": null,
      "account_id": 3,
      "created_at": "2026-06-28T10:00:00Z"
    }
  ],
  "meta": { "page": 1, "per_page": 25, "total": 1, "total_pages": 1 }
}
```

### `GET /api/v1/scheduled_emails/:id` — show (scope `scheduled_emails:read`)

Adds the full `body` (HTML).

### `POST /api/v1/scheduled_emails` — create (scope `scheduled_emails:write`)

Body: `email_account_id` (required), `to_address` (required), `subject`, `body`,
`cc_address`, `bcc_address`, `scheduled_at`, `rrule` (iCal RRULE string, e.g.
`FREQ=WEEKLY;INTERVAL=1`). Returns `201`.

### `PATCH /api/v1/scheduled_emails/:id` — update (scope `scheduled_emails:write`)

Same fields as create; only the fields you send change.

### `DELETE /api/v1/scheduled_emails/:id` — cancel (scope `scheduled_emails:write`)

Soft-cancels the scheduled email (`status: "cancelled"`). Returns the updated record.

## Calendar Events

Calendar events are scoped to events the acting user may see via their connected
calendar accounts. Writes require the target calendar to be writable and enabled
for sync (`403 calendar_not_writable`); modifying an event on a non-writable
calendar returns `403 event_not_writable`.

### `GET /api/v1/calendar_events` — list (scope `calendar:read`)

Filters: `start_after`, `start_before` (ISO 8601), `calendar_id`, plus
`page`/`per_page`. Ordered by start time ascending.

```json
{
  "data": [
    {
      "id": 55, "title": "Team sync", "location": "Google Meet",
      "start_at": "2026-07-01T10:00:00Z", "end_at": "2026-07-01T11:00:00Z",
      "all_day": false, "status": "confirmed", "rsvp_status": "accepted",
      "color": "#4285f4", "calendar_id": 2,
      "conference_url": "https://meet.google.com/abc-def-ghi",
      "html_link": "https://calendar.google.com/event?eid=...",
      "is_organizer": true, "recurring": false,
      "source_email_message_id": null,
      "created_at": "2026-06-23T09:00:00Z"
    }
  ],
  "meta": { "page": 1, "per_page": 25, "total": 1, "total_pages": 1 }
}
```

### `GET /api/v1/calendar_events/:id` — show (scope `calendar:read`)

Adds `description`, `attendees` (array), and `rrule`.

### `POST /api/v1/calendar_events` — create (scope `calendar:write`)

Body: `calendar_id` (required, must be a writable calendar the user can access),
`title` (required), `start_at` (required, ISO 8601), `end_at`, `description`,
`location`, `all_day`, `color` (hex). Returns `201`. Returns `403
calendar_not_writable` if the calendar is not writable or does not exist.

### `PATCH /api/v1/calendar_events/:id` — update (scope `calendar:write`)

Body: `title`, `start_at`, `end_at`, `description`, `location`, `all_day`, `color`.
Queues an async provider update. Returns `403 event_not_writable` when the event's
calendar is not writable.

### `DELETE /api/v1/calendar_events/:id` — delete (scope `calendar:write`)

Provider deletion is asynchronous. Returns **`202 Accepted`** with the current
event state. The local record is tombstoned (`status: "cancelled"`) once the
provider confirms.

### `POST /api/v1/calendar_events/:id/rsvp` — RSVP (scope `calendar:write`)

Body: `rsvp_status` — one of `needs_action`, `accepted`, `declined`,
`tentative`. Returns `422 invalid_rsvp_status` for unrecognized values.

## Reminders

Reminders are AI-extracted automatically from emails and documents — there is
**no create endpoint**. They are scoped to the acting user.

### `GET /api/v1/reminders` — list (scope `reminders:read`)

Filter by `status` (`pending`/`confirmed`/`dismissed`/`snoozed`), plus
`page`/`per_page`. Ordered by `due_at` ascending.

```json
{
  "data": [
    {
      "id": 9, "title": "Submit VAT return",
      "description": "VAT return due by end of month",
      "due_at": "2026-07-31T23:59:00Z", "all_day": true,
      "reminder_type": "deadline", "status": "pending",
      "confidence": 0.92, "amount_cents": null, "currency": null,
      "snoozed_until": null, "source_type": "EmailMessage",
      "source_id": 42, "calendar_event_id": null,
      "created_at": "2026-06-23T09:00:00Z"
    }
  ],
  "meta": { "page": 1, "per_page": 25, "total": 1, "total_pages": 1 }
}
```

### `GET /api/v1/reminders/:id` — show (scope `reminders:read`)

Adds `justification` (AI reasoning text) and `extracted_data` (raw extraction hash).

### `POST /api/v1/reminders/:id/confirm` — confirm (scope `reminders:write`)

Marks the reminder confirmed and creates a calendar event (if a writable calendar
is available). Optional body: `due_at` (ISO 8601) to override the extracted due
date. Returns the reminder plus `calendar_event_id` of the newly created event
(or `null` if no writable calendar).

### `POST /api/v1/reminders/:id/dismiss` — dismiss (scope `reminders:write`)

Marks the reminder dismissed. Returns the updated reminder.

### `POST /api/v1/reminders/:id/snooze` — snooze (scope `reminders:write`)

Body: optional `until` (ISO 8601; defaults to one week from now). Returns the
updated reminder.

## Tasks

Tasks are actionable items — created manually, via this API, or AI-extracted from
emails and documents (extracted ones arrive in `suggested` status for triage).
Scoped to the acting user's workspace.

### `GET /api/v1/tasks` — list (scope `tasks:read`)

Filter by `status` (`suggested`/`todo`/`in_progress`/`blocked`/`done`/`cancelled`)
and `assignee_id` (and `archived=true` to list archived tasks instead of active
ones), plus `page`/`per_page`. Ordered by `created_at` descending.

```json
{
  "data": [
    {
      "id": "bc1d5f02-f40e-4f57-9d61-2914cbe3e4ae",
      "title": "Send Q3 report to the board",
      "description": "<p>Pull the numbers and email the deck.</p>",
      "status": "in_progress", "priority": "high",
      "due_at": "2026-07-01T09:00:00Z", "all_day": false,
      "completed_at": null, "archived_at": null, "ai_suggested": false,
      "source_type": null, "source_id": null,
      "created_by_id": "1f2e…", "assignee_ids": ["9a8b…"], "tag_ids": ["3c4d…"],
      "created_at": "2026-06-29T02:13:00Z", "updated_at": "2026-06-29T02:16:00Z"
    }
  ],
  "meta": { "page": 1, "per_page": 25, "total": 1, "total_pages": 1 }
}
```

### `GET /api/v1/tasks/:id` — show (scope `tasks:read`)

Adds `justification`, `confidence`, `linked_emails` (typed email links),
`document_ids` (attached documents), and `extracted_data`.

### `POST /api/v1/tasks` — create (scope `tasks:write`)

Body: `title` (required), plus optional `description`, `priority`
(`low`/`normal`/`high`/`urgent`), `due_at` (ISO 8601), `all_day`, `assignee_ids`,
`tag_ids`, and an initial `status` (defaults to `todo`). Returns the created task (201).

### `PATCH /api/v1/tasks/:id` — update (scope `tasks:write`)

Same fields as create. Passing a new `status` performs a tracked transition
(publishing `task.status_changed` / `task.completed`).

### `PATCH /api/v1/tasks/:id/complete` — complete (scope `tasks:write`)

Marks the task `done` and stamps `completed_at`. Returns the updated task.

## Folders

Folders are workspace-scoped custom mail folders. The list is returned
**unpaginated** (no `meta`). Folder create/rename/delete have provider side-effects
and are not exposed here.

### `GET /api/v1/folders` — list (scope `folders:read`)

```json
{
  "data": [
    {
      "id": 1, "name": "Clients", "icon": "folder",
      "parent_id": null, "position": 0,
      "document_count": 14, "created_at": "2026-05-01T09:00:00Z"
    }
  ]
}
```

### `GET /api/v1/folders/:id` — show (scope `folders:read`)

Adds `documents` (array) — the full document list for documents filed in this
folder, newest first.

### `POST /api/v1/folder_memberships` — file a document (scope `folders:write`)

Body: `mail_folder_id` (required), `document_id` (required). Files the document
into the folder. Idempotent — filing the same document twice returns the same
membership. Returns `201` with
`{ "data": { "id": ..., "folder_id": ..., "document_id": ... } }`.

### `DELETE /api/v1/folder_memberships/:id` — unfile a document (scope `folders:write`)

Removes the document from the folder. Returns `204 No Content`.

## MCP endpoint

`POST /api/mcp` exposes the same data as the REST API as a **JSON-RPC 2.0
(Model Context Protocol)** server, so MCP-capable AI clients (LLM agents, IDEs,
Claude Desktop, etc.) can call Campbooks tools directly.

Authentication is the **same Doorkeeper bearer token** as the REST API:

```
Authorization: Bearer YOUR_ACCESS_TOKEN
```

Configure an MCP client to point at `https://<your-campbooks-host>/api/mcp`
with that header. OAuth 2.1 dynamic registration and SSE streaming are **not**
yet supported.

Protocol version: `2025-03-26`.

### Initialize

Request:
```json
{
  "jsonrpc": "2.0", "id": 1, "method": "initialize",
  "params": {
    "protocolVersion": "2025-03-26",
    "capabilities": {},
    "clientInfo": { "name": "my-agent", "version": "1.0" }
  }
}
```

Response:
```json
{
  "jsonrpc": "2.0", "id": 1,
  "result": {
    "protocolVersion": "2025-03-26",
    "capabilities": { "tools": {} },
    "serverInfo": { "name": "campbooks", "version": "0.2.1" }
  }
}
```

### tools/list

Only tools whose required scope the token holds are included. Send a
`notifications/initialized` notification after `initialize` to complete the
handshake.

Request:
```json
{ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {} }
```

Response (excerpt):
```json
{
  "jsonrpc": "2.0", "id": 2,
  "result": {
    "tools": [
      {
        "name": "list_emails",
        "description": "List the most recent emails...",
        "inputSchema": { "type": "object", "properties": { "limit": { "type": "integer" } } }
      }
    ]
  }
}
```

### tools/call

Content is an array of `{ "type": "text", "text": "..." }` items. Tool failures
set `isError: true` instead of using a JSON-RPC error code.

Request:
```json
{
  "jsonrpc": "2.0", "id": 3, "method": "tools/call",
  "params": { "name": "list_emails", "arguments": { "unread": true, "limit": 5 } }
}
```

Response:
```json
{
  "jsonrpc": "2.0", "id": 3,
  "result": {
    "content": [{ "type": "text", "text": "{\"emails\":[...]}" }]
  }
}
```

Tool error (tool was called successfully but the tool itself reports an error):
```json
{
  "jsonrpc": "2.0", "id": 3,
  "result": {
    "content": [{ "type": "text", "text": "Not found." }],
    "isError": true
  }
}
```

### Protocol error codes

| Code | When |
|------|------|
| `-32700` | Parse error -- request body was not valid JSON |
| `-32600` | Invalid Request -- missing `jsonrpc`/`method` fields |
| `-32601` | Method not found -- unrecognized method name |
| `-32602` | Invalid params -- a required tool argument was missing |
| `-32000` | Insufficient scope -- the token lacks the scope required by the tool |

### Available tools

| Tool | Required scope | Description |
|------|---------------|-------------|
| `list_emails` | `emails:read` | List recent emails, filtered by unread/query |
| `get_email` | `emails:read` | Fetch a single email by ID including its body |
| `send_email` | `emails:send` | Send a new email from a connected account |
| `reply_email` | `emails:send` | Reply to an existing email |
| `mark_email_read` / `mark_email_unread` | `emails:write` | Toggle an email's read flag |
| `add_email_tag` / `remove_email_tag` | `tags:write` | Attach/detach a tag on an email |
| `list_documents` / `get_document` | `documents:read` | List documents / fetch one with fields + file info |
| `upload_document` | `documents:write` | Upload a document from base64 content |
| `update_document` | `documents:write` | Edit a document's extracted fields |
| `approve_document` / `reject_document` / `reclassify_document` | `documents:write` | Change a document's review state |
| `list_contacts` / `get_contact` | `contacts:read` | List/fetch contacts |
| `update_contact` / `set_contact_state` | `contacts:write` | Edit a contact / star, block, allow |
| `list_tags` | `tags:read` | List workspace tags |
| `list_document_types` | `document_types:read` | List document types |
| `list_workflows` / `list_workflow_executions` | `workflows:read` | List workflows / run history (feature-gated) |
| `trigger_workflow` | `workflows:trigger` | Trigger a webhook workflow (feature-gated) |
| `list_scout_threads` / `list_scout_messages` | `scout:read` | Read Scout chat |
| `create_scout_thread` / `send_scout_message` | `scout:write` | Start a thread / post a message (async reply) |
| `list_scheduled_emails` / `get_scheduled_email` | `scheduled_emails:read` | List/fetch scheduled emails |
| `create_scheduled_email` / `update_scheduled_email` / `cancel_scheduled_email` | `scheduled_emails:write` | Schedule, edit, cancel |
| `list_calendar_events` / `get_calendar_event` | `calendar:read` | List/fetch calendar events |
| `create_calendar_event` / `update_calendar_event` / `delete_calendar_event` / `rsvp_calendar_event` | `calendar:write` | Create, edit, delete, RSVP |
| `list_reminders` / `get_reminder` | `reminders:read` | List/fetch reminders |
| `confirm_reminder` / `dismiss_reminder` / `snooze_reminder` | `reminders:write` | Act on a reminder |
| `list_folders` / `get_folder` | `folders:read` | List folders / fetch a folder with its documents |
| `file_document` / `unfile_document` | `folders:write` | File/unfile a document in a folder |

The MCP surface mirrors the REST API one-for-one — same auth, same scopes, same
permission checks. Workflow tools appear in `tools/list` only when the Workflows
feature is enabled server-side (`ENABLE_WORKFLOWS`).

## Code samples

### Python

```python
import requests

HOST = "https://<your-campbooks-host>"

# 1. Get a token.
token = requests.post(f"{HOST}/api/oauth/token", data={
    "grant_type": "client_credentials",
    "client_id": "YOUR_CLIENT_ID",
    "client_secret": "YOUR_CLIENT_SECRET",
    "scope": "emails:read",
}).json()["access_token"]

# 2. Call the API.
headers = {"Authorization": f"Bearer {token}"}
emails = requests.get(f"{HOST}/api/v1/emails", headers=headers,
                      params={"unread": True, "per_page": 50}).json()
for email in emails["data"]:
    print(email["received_at"], email["from"], email["subject"])
```

### JavaScript (Node, fetch)

```js
const HOST = "https://<your-campbooks-host>";

const tokenRes = await fetch(`${HOST}/api/oauth/token`, {
  method: "POST",
  headers: { "Content-Type": "application/x-www-form-urlencoded" },
  body: new URLSearchParams({
    grant_type: "client_credentials",
    client_id: "YOUR_CLIENT_ID",
    client_secret: "YOUR_CLIENT_SECRET",
    scope: "emails:read",
  }),
});
const { access_token } = await tokenRes.json();

const res = await fetch(`${HOST}/api/v1/emails?unread=true`, {
  headers: { Authorization: `Bearer ${access_token}` },
});
const { data } = await res.json();
console.log(data.map((e) => `${e.from}: ${e.subject}`));
```

### Scout poll loop (Python)

```python
# Post a message, then poll until the AI reply appears.
msg = requests.post(f"{HOST}/api/v1/scout/threads/7/messages", headers=headers,
                    data={"content": "What needs my attention today?"}).json()["data"]

while True:
    new = requests.get(f"{HOST}/api/v1/scout/threads/7/messages",
                       headers=headers,
                       params={"after_message_id": msg["id"]}).json()["data"]
    reply = next((m for m in new if m["author_type"] == "ai"
                  and m["reply_status"] == "replied"), None)
    if reply:
        print(reply["content"])
        break
    time.sleep(2)
```

## Versioning

The API is versioned in the path (`/api/v1`). Additive changes (new fields,
new endpoints, new scopes) may ship within a version; breaking changes get a new
version. The [`openapi.yaml`](../openapi.yaml) spec is kept in lockstep with
this guide.
