# Campbooks Public REST API

The public API lets customers reach their own Campbooks data — email,
documents, contacts, tags, document types, and Scout chat — from
their own apps and scripts. It is authenticated with **OAuth 2.0 client
credentials** via [Doorkeeper](https://github.com/doorkeeper-gem/doorkeeper).

> Replace `https://<your-campbooks-host>` below with your deployment's host
> (e.g. the value of `APP_HOST`). All endpoints are served over HTTPS in
> production.

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

<!-- The `workflows:read` / `workflows:trigger` scopes are omitted while the Workflows feature is disabled by default (ENABLE_WORKFLOWS). Restore both rows above when it ships. -->

Scopes are a ceiling, not a grant of new power: a request must satisfy **both**
the token's scope **and** the acting user's own permissions (e.g. `emails:send`
still requires that the user may send from the chosen account).

## Conventions

- **Base path:** `/api/v1`
- **Format:** JSON. Collections are `{ "data": [ … ], "meta": { … } }`; single
  resources are `{ "data": { … } }`.
- **Pagination:** `?page=` and `?per_page=` (default 25, max 100). `meta`
  carries `page`, `per_page`, `total`, `total_pages`.
- **Rate limit:** 600 requests/minute per client (HTTP 429 when exceeded).
- **Errors:** `{ "error": { "code": "…", "message": "…" } }`, with validation
  details under `error.details` where relevant.

| Status | `error.code` | When |
|--------|--------------|------|
| 401 | `invalid_token` | Missing/invalid/expired/revoked token |
| 401 | `client_revoked` | The client's workspace/user no longer exists or matches |
| 403 | `insufficient_scope` | Token lacks the scope (or has none) for the action |
| 400 | `missing_parameter` | A required parameter is absent |
| 404 | `not_found` | The resource doesn't exist **or isn't visible to you** |
| 422 | `validation_failed` | The payload was rejected (`error.details`) |
| 429 | `rate_limit_exceeded` | Too many requests |

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
mailbox.

### `POST /api/v1/emails` — send (scope `emails:send`)

Body: `email_account_id` (required), `to_address` (required), `subject`, `body`,
`cc_address`, `bcc_address`. Returns `201` with
`{ "data": { "id": …, "provider_message_id": "…" } }`.

### `POST /api/v1/emails/:id/reply` — reply (scope `emails:send`)

Body: `body` (required), optional `to_address` (defaults to the original
sender), `cc_address`, `bcc_address`, `email_account_id` (defaults to the source
message's account). Threads automatically.

## Documents

### `GET /api/v1/documents` — list (scope `documents:read`)

Filters: `type` (document type id), `review_status` (`pending`/`approved`/
`rejected`), `ai_status` (`pending`/`processing`/`completed`/`failed`), plus
`page`/`per_page`. Documents are workspace-wide (every member sees them).

### `GET /api/v1/documents/:id` — show (scope `documents:read`)

Includes extracted fields, the attached file's metadata, and the raw AI
`extraction` data.

### `GET /api/v1/documents/:id/file` — download (scope `documents:read`)

Streams the original uploaded file (send the same bearer token).

### `POST /api/v1/documents` — upload (scope `documents:write`)

Multipart `files[]` (one or more). AI classification/extraction runs
asynchronously, so the response is **`202 Accepted`** and each document starts
with `ai_status: "pending"`.

```bash
curl -X POST https://<your-campbooks-host>/api/v1/documents \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN" \
  -F "files[]=@invoice.pdf"
```

### `PATCH /api/v1/documents/:id` — update fields (scope `documents:write`)

Top-level body with any extracted fields: `document_type_id`, `vendor_name`,
`invoice_number`, `amount_cents`, `currency`, `document_date`, `description`,
`metadata`, etc. Editing fields does not change review state.

### `POST /api/v1/documents/:id/approve` · `…/reject` (scope `documents:write`)

Approve or reject a document under review; approve records the acting user as the reviewer.

### `POST /api/v1/documents/:id/reclassify` — change type (scope `documents:write`)

Body: `document_type_id` (required). Reclassifying also signs the document off (it becomes `approved`).

## Contacts

Contacts are created automatically from email sync — there is **no create endpoint**. They are workspace-wide.

### `GET /api/v1/contacts` — list (scope `contacts:read`)

Filters: `list_status` (`neutral`/`allowed`/`blocked`), `starred` (bool), `q` (matches name/email), plus `page`/`per_page`.

### `GET /api/v1/contacts/:id` — show (scope `contacts:read`)

### `PATCH /api/v1/contacts/:id` — update (scope `contacts:write`)

Body: `name`, `relationship_type` (one of self/client/vendor/partner/service_provider/colleague/personal/unknown). Only the fields you send change.

### `POST /api/v1/contacts/:id/state` — change state (scope `contacts:write`)

Body: `state` — one of `star`, `unstar`, `allow`, `block`, `unblock`. Blocking also archives the contact's inbox messages.

## Tags

Tags apply to **emails only** (documents are organized by document type instead).

### `GET /api/v1/tags` — list (scope `tags:read`)

### `POST /api/v1/emails/:id/tags` — add a tag (scope `tags:write`)

Body: `tag_id` **or** `name` (must be an existing workspace tag — tags aren't created here).

### `DELETE /api/v1/emails/:id/tags/:tag_id` — remove a tag (scope `tags:write`)

## Document types

### `GET /api/v1/document_types` — list (scope `document_types:read`)

Returns the workspace's document types (`id`, `name`, `color`, `category`, `auto_star`, `extraction_schema`) — use a type's `id` with document upload/update and reclassify.

<!-- TEMPORARILY DISABLED: the Workflows feature ships gated off by default (ENABLE_WORKFLOWS); these endpoints return 404 until it's enabled. Restore this section — plus the workflows:* scope rows and "workflows" in the intro above — when the feature ships.

## Workflows

Workflows are workspace-wide automations. The API can list them, read their run
history, and trigger **webhook** workflows on demand (the authenticated
equivalent of the public `POST /webhooks/:token` endpoint).

### `GET /api/v1/workflows` — list (scope `workflows:read`)

Returns `id`, `name`, `description`, `trigger_type` (`email_received`/`webhook`/
`event`), `enabled`, and `webhook_token` (present for webhook workflows — combine
with your host as `https://<your-campbooks-host>/webhooks/<webhook_token>` for the
no-auth inbound URL). Paginated.

### `GET /api/v1/workflows/:id/executions` — run history (scope `workflows:read`)

Lists the workflow's runs newest-first: `id`, `status` (`running`/`completed`/
`failed`), `started_at`, `completed_at`, `error_message`, and `trigger_data` (the
payload/context the run started from). Paginated.

### `POST /api/v1/workflows/:id/trigger` — trigger (scope `workflows:trigger`)

Fires a workflow asynchronously. Body: an optional `payload` object, exposed to
the workflow's Liquid templates exactly like an inbound webhook body. Returns
**`202 Accepted`**; the run happens in the background (poll the executions
endpoint to watch it).

```bash
curl -X POST https://<your-campbooks-host>/api/v1/workflows/42/trigger \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"payload": {"status": "paid", "invoice": "INV-1"}}'
```

Only **webhook** workflows can be triggered this way — triggering a disabled
workflow returns `422 workflow_disabled`, and a non-webhook one returns
`422 not_triggerable`.
-->

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

Returns the thread's messages in chronological order. Pass
`?after_message_id=N` to fetch only messages created **after** message `N` — the
poll loop for the async reply. Each message carries `id`, `author_type`
(`user`/`ai`), `content`, `reply_status` (`pending`/`processing`/`replied`/
`failed`), `suggested_actions`, and `prompts`.

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

## Versioning

The API is versioned in the path (`/api/v1`). Additive changes (new fields,
new endpoints, new scopes) may ship within a version; breaking changes get a new
version.
