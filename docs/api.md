# Campbooks Public REST API

The public API lets customers reach their own Campbooks data — email,
documents, contacts, tags, document types, workflows, and Scout chat — from
their own apps and scripts. It is authenticated with **OAuth 2.0 client
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
| `workflows:read` | List workflows and their run history |
| `workflows:trigger` | Trigger a webhook workflow |
| `scout:read` | Read Scout chat threads and messages |
| `scout:write` | Create Scout threads and send messages |

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
| 422 | `workflow_disabled` | `POST /workflows/:id/trigger`: the workflow is disabled |
| 422 | `not_triggerable` | `POST /workflows/:id/trigger`: not a webhook workflow |
| 429 | `rate_limit_exceeded` | Too many requests |
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

## Workflows

Workflows are workspace-wide automations. The API can list them, read their run
history, and trigger **webhook** workflows on demand (the authenticated
equivalent of the public `POST /webhooks/:token` endpoint).

### `GET /api/v1/workflows` — list (scope `workflows:read`)

Returns `id`, `name`, `description`, `trigger_type` (`email_received`/`webhook`/
`event`), `enabled`, `webhook_token` (present for webhook workflows — combine
with your host as `https://<your-campbooks-host>/webhooks/<webhook_token>` for the
no-auth inbound URL), plus `created_at`/`updated_at`. Paginated.

```json
{
  "data": [
    {
      "id": 12, "name": "Invoice paid → Slack", "description": null,
      "trigger_type": "webhook", "enabled": true,
      "webhook_token": "wh_abc123", "created_at": "2026-06-10T08:00:00Z",
      "updated_at": "2026-06-21T08:00:00Z"
    }
  ],
  "meta": { "page": 1, "per_page": 25, "total": 1, "total_pages": 1 }
}
```

### `GET /api/v1/workflows/:id/executions` — run history (scope `workflows:read`)

Lists the workflow's runs newest-first: `id`, `workflow_id`, `status`
(`running`/`completed`/`failed`), `started_at`, `completed_at`,
`error_message`, `trigger_data` (the payload/context the run started from), and
`created_at`. Paginated.

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
