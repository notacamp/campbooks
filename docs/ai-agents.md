# AI agents & MCP

Campbooks exposes a [Model Context Protocol](https://modelcontextprotocol.io) endpoint at
`POST /api/mcp`. MCP-capable AI agents — Claude Code, Cursor, Windsurf, OpenAI Codex CLI,
Gemini CLI, and any client that speaks streamable HTTP JSON-RPC — can connect to your inbox,
triage email, review documents, manage tasks and calendar events, and run the Skim loop in
plain language, without writing a line of code.

## What you get

An agent connected to Campbooks can:

- **Read and triage email** — list, search, read, archive, pin, snooze, and trash threads;
  apply decisions to whole clusters at once via the Skim loop.
- **Send and reply** — draft and send from any connected account the acting user may send from,
  reply to threads, and forward messages.
- **Review documents** — list invoices, receipts, and contracts; read extracted fields; approve,
  reject, or reclassify pending documents.
- **Manage tasks and calendar** — create, update, and complete tasks; read and write calendar
  events and RSVP on invites; confirm AI-extracted reminders into calendar events.
- **Organise** — add and remove email tags; move emails to folders; file documents into folders.
- **Ask Scout** — post messages to Scout chat threads and poll for the AI reply; Scout has
  access to the full inbox and document context.
- **Connect accounts** — kick off a mailbox OAuth flow from the agent.
- **Schedule email** — create and manage one-off and recurring scheduled sends.
- **Automate** — list and trigger webhook workflows (when `ENABLE_WORKFLOWS=1`).

### Minimal-context design

The server is designed so agents consume as little context as possible. Three meta tools are
available to every authenticated client regardless of scope:

- **`get_overview`** — a cheap snapshot (unread count, pending documents, today's calendar,
  overdue reminders, awaiting-reply count). Call this first to orient the session.
- **`get_setup_status`** — workspace onboarding state; useful at the start of a setup flow.
- **`guide(topic)`** — on-demand narrative guides. Call with no topic to list the available
  ones; topics include `triage_and_skim`, `documents`, `tasks_and_calendar`, `sending_email`,
  `organizing`, `setup_and_accounts`, `context_tips`, `automation`, and `getting_started`.
  Loaded only when the agent actually needs them, so they cost nothing when they are not.

Scope selection also controls which tools appear in `tools/list`: a triage-only client with
`emails:read emails:write` receives only the tools it needs, which reduces context per session.

### Skim learning loop

When an agent calls `skim_decide`, the decision is fed back to the Skim learning loop — the
same loop that drives the in-app Skim tray. Decisions accumulate over time and improve future
cluster suggestions and priority scoring, just as they do when a user triages from the inbox UI.

## MCP endpoint basics

```
POST https://<your-campbooks-host>/api/mcp
```

The endpoint speaks [JSON-RPC 2.0](https://www.jsonrpc.org/specification) over streamable HTTP
(protocol version `2025-03-26`). It is stateless — send `initialize` at the start of each
session.

### Typical session

```json
// 1. Initialize
{
  "jsonrpc": "2.0", "id": 1, "method": "initialize",
  "params": {
    "protocolVersion": "2025-03-26",
    "capabilities": {},
    "clientInfo": { "name": "my-agent", "version": "1.0" }
  }
}

// 2. Notify initialized (no response expected)
{ "jsonrpc": "2.0", "method": "notifications/initialized" }

// 3. List tools — only the tools the token's scopes permit are returned
{ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {} }

// 4. Call a tool
{
  "jsonrpc": "2.0", "id": 3, "method": "tools/call",
  "params": { "name": "get_overview", "arguments": {} }
}
```

Wire-level details — `initialize`/`tools/list`/`tools/call` schemas, error codes, and
supported `Authorization` forms — are in [`docs/api.md`](api.md#mcp-endpoint).

## Creating an MCP key

Short-lived bearer tokens (valid 2 hours) expire between agent sessions. MCP keys do not
expire, making them the right credential for static agent configurations.

1. Open **Settings → API access** and click **New client**.
2. Give it a name (e.g. "Claude Code agent") and select the scopes it needs.
3. Click **Create**. The next screen shows the **MCP key** — a single string in the form
   `<client-id>.<client-secret>`. Copy it now; it is shown only once. If you lose it,
   regenerate the client secret from the same Settings page.
4. Paste the key into your agent's configuration as the Bearer credential (see the sections
   below for each agent's exact config format).

**Revocation:** rotate the key by regenerating the client secret in Settings → API access.
Revoking individual access tokens does not invalidate an MCP key; deleting the client or
regenerating its secret does.

**Classic tokens:** short-lived tokens from `POST /api/oauth/token` also work at `/api/mcp`.
Use them if your agent runtime handles token refresh automatically; otherwise prefer the
MCP key.

### Recommended scope sets

**Full set** — triage, documents, contacts, tasks, calendar, scheduling, Scout, and account
management:

```
emails:read emails:write emails:send
tags:read tags:write
documents:read documents:write
document_types:read document_types:write
contacts:read contacts:write
calendar:read calendar:write
reminders:read reminders:write
tasks:read tasks:write
folders:read folders:write
email_accounts:read email_accounts:write
scout:read scout:write
scheduled_emails:read scheduled_emails:write
```

**Minimal triage-only set** — read and archive/tag, no send, no documents:

```
emails:read emails:write
tags:read
```

The three meta tools (`get_overview`, `get_setup_status`, `guide`) require no scope and
appear for any authenticated client regardless of what scopes the key holds.

## Claude Code (the plugin)

The official Claude Code plugin wraps the MCP endpoint and adds the `/campbooks:setup` and
`/campbooks:triage` skills.

### Install

```
/plugin marketplace add notacamp/campbooks
/plugin install campbooks@campbooks
```

Claude Code will prompt for two values:

- **Campbooks server URL** — `https://app.campbooks.not-a-camp.com` for Campbooks Cloud, or
  your self-hosted URL (no trailing slash). The plugin appends `/api/mcp` automatically.
- **MCP key** — create one as described above.

### Skills

**`/campbooks:setup`** — guided onboarding: verifies the MCP connection, connects a mailbox,
configures AI parsing, bootstraps document types and tags, and walks through the first Skim
session. Invoke when the plugin is not yet connected, or to add a new mailbox.

**`/campbooks:triage`** — daily inbox run: overview → Skim deck → awaiting-reply threads →
pending documents → suggested tasks and reminders → closing summary. The skill asks for
confirmation before every write — it will never archive, send, or block without naming exactly
what it is about to do and waiting for an explicit yes from you.

## Other agents

The MCP endpoint is plain streamable HTTP with a Bearer credential — any MCP-capable client
works. Replace `YOUR_MCP_KEY_HERE` / `$CAMPBOOKS_MCP_KEY` with your MCP key or an environment
variable that holds it.

For self-hosted instances, replace `https://app.campbooks.not-a-camp.com` with your server URL
in each snippet.

### Cursor (`~/.cursor/mcp.json`)

```json
{
  "mcpServers": {
    "campbooks": {
      "type": "http",
      "url": "https://app.campbooks.not-a-camp.com/api/mcp",
      "headers": {
        "Authorization": "Bearer ${env:CAMPBOOKS_MCP_KEY}"
      }
    }
  }
}
```

### Windsurf (`mcp_config.json`)

```json
{
  "mcpServers": {
    "campbooks": {
      "type": "http",
      "url": "https://app.campbooks.not-a-camp.com/api/mcp",
      "headers": {
        "Authorization": "Bearer ${env:CAMPBOOKS_MCP_KEY}"
      }
    }
  }
}
```

### OpenAI Codex CLI (`config.toml`)

```toml
[mcp_servers.campbooks]
url = "https://app.campbooks.not-a-camp.com/api/mcp"
bearer_token_env_var = "CAMPBOOKS_MCP_KEY"
```

### Gemini CLI (`settings.json`)

```json
{
  "mcpServers": {
    "campbooks": {
      "httpUrl": "https://app.campbooks.not-a-camp.com/api/mcp",
      "headers": {
        "Authorization": "Bearer YOUR_MCP_KEY_HERE"
      }
    }
  }
}
```

## Tool catalog

All 72 tools are listed below, grouped by family. Tools appear in `tools/list` only for the
scopes the client holds. The three meta tools (`get_overview`, `get_setup_status`, `guide`)
require no scope.

### Meta

| Tool | Purpose | Scope |
|------|---------|-------|
| `get_overview` | Cheap snapshot of what needs attention. Call this first. | (any) |
| `get_setup_status` | Workspace setup snapshot for onboarding and diagnostics. | (any) |
| `guide` | Narrative guides for working with Campbooks over MCP. | (any) |

### Email read

| Tool | Purpose | Scope |
|------|---------|-------|
| `list_emails` | List the most recent emails the caller can access, newest first. | `emails:read` |
| `search_emails` | Search emails with filters. | `emails:read` |
| `get_email` | Fetch a single email by id. | `emails:read` |

### Email act

| Tool | Purpose | Scope |
|------|---------|-------|
| `send_email` | Send a new email from one of the caller's connected accounts. | `emails:send` |
| `reply_email` | Reply to an existing email. | `emails:send` |
| `forward_email` | Forward an email to another address. | `emails:send` |
| `mark_email_read` | Mark an email as read and sync the flag to the provider mailbox. | `emails:write` |
| `mark_email_unread` | Mark an email as unread (local only). | `emails:write` |
| `update_emails` | Bulk-act on emails. | `emails:write` |
| `move_emails_to_folder` | Move emails (and their full threads) to a folder. | `emails:write` |

### Skim

| Tool | Purpose | Scope |
|------|---------|-------|
| `get_skim_deck` | Return the Skim inbox deck as compact rings and cluster cards. | `emails:read` |
| `skim_decide` | Apply a Skim triage decision to a cluster's emails. | `emails:write` |

### Accounts

| Tool | Purpose | Scope |
|------|---------|-------|
| `list_email_accounts` | List connected email accounts visible to the caller. | `email_accounts:read` |
| `connect_email_account` | Connect a new email account. | `email_accounts:write` |

### Documents

| Tool | Purpose | Scope |
|------|---------|-------|
| `list_documents` | List the workspace's documents, newest first. | `documents:read` |
| `get_document` | Fetch a document by id with its extracted fields and file info. | `documents:read` |
| `upload_document` | Upload a new document from base64 content. | `documents:write` |
| `update_document` | Edit a document's extracted fields. | `documents:write` |
| `approve_document` | Approve (sign off) a document. | `documents:write` |
| `reject_document` | Reject a document. | `documents:write` |
| `reclassify_document` | Change a document's type (also approves it). | `documents:write` |

### Contacts

| Tool | Purpose | Scope |
|------|---------|-------|
| `list_contacts` | List the workspace's contacts. | `contacts:read` |
| `get_contact` | Fetch a single contact by id. | `contacts:read` |
| `update_contact` | Update a contact's name and/or relationship type. | `contacts:write` |
| `set_contact_state` | Star/unstar, allow, block, or unblock a contact. | `contacts:write` |

### Tags, types & folders

| Tool | Purpose | Scope |
|------|---------|-------|
| `list_tags` | List the workspace's tags (tags apply to emails). | `tags:read` |
| `create_tag` | Create a new workspace tag. | `tags:write` |
| `add_email_tag` | Attach an existing workspace tag to an email (by tag_id or name). | `tags:write` |
| `remove_email_tag` | Detach a tag from an email. | `tags:write` |
| `tag_emails` | Add or remove a tag on a set of emails. | `tags:write` |
| `list_document_types` | List the workspace's document types (used to classify documents). | `document_types:read` |
| `create_document_type` | Create a new document type for classifying attachments. | `document_types:write` |
| `list_folders` | List the workspace's custom folders. | `folders:read` |
| `get_folder` | Fetch a folder and the documents filed into it. | `folders:read` |
| `create_folder` | Create a custom folder. | `folders:write` |
| `file_document` | File a document into a folder. | `folders:write` |
| `unfile_document` | Remove a document from a folder (by membership id). | `folders:write` |
| `list_email_templates` | List the workspace's reusable email templates. | `templates:read` ¹ |

> ¹ `list_email_templates` appears only when `ENABLE_EMAIL_TEMPLATES=1`.

### Tasks

| Tool | Purpose | Scope |
|------|---------|-------|
| `list_tasks` | List workspace tasks. | `tasks:read` ¹ |
| `get_task` | Fetch a task by id with full detail. | `tasks:read` ¹ |
| `create_task` | Create a task in the workspace. | `tasks:write` ¹ |
| `update_task` | Update a task's fields. | `tasks:write` ¹ |
| `complete_task` | Mark a task as done. | `tasks:write` ¹ |
| `create_task_from_email` | Extract and create a task from an email via the action registry. | `tasks:write` ¹ |

> ¹ Task tools appear only when `ENABLE_TASKS=1`.

### Calendar

| Tool | Purpose | Scope |
|------|---------|-------|
| `list_calendars` | List calendars visible to the caller. | `calendar:read` |
| `list_calendar_events` | List calendar events the caller can access, soonest first. | `calendar:read` |
| `get_calendar_event` | Fetch a calendar event by id. | `calendar:read` |
| `create_calendar_event` | Create a calendar event on one of the caller's writable calendars. | `calendar:write` |
| `update_calendar_event` | Update a calendar event (you must have write access to its calendar). | `calendar:write` |
| `delete_calendar_event` | Delete a calendar event (async provider delete). | `calendar:write` |
| `rsvp_calendar_event` | Set your RSVP on an event (needs_action, accepted, declined, tentative). | `calendar:write` |
| `create_event_from_email` | Extract and create a calendar event from an email. | `calendar:write` |

### Reminders

| Tool | Purpose | Scope |
|------|---------|-------|
| `list_reminders` | List AI-extracted reminders the caller can access. | `reminders:read` |
| `get_reminder` | Fetch a reminder by id. | `reminders:read` |
| `confirm_reminder` | Confirm a reminder into a calendar event. | `reminders:write` |
| `dismiss_reminder` | Dismiss a reminder. | `reminders:write` |
| `snooze_reminder` | Snooze a reminder until the given time, or one week out when omitted. | `reminders:write` |

### Scheduled emails

| Tool | Purpose | Scope |
|------|---------|-------|
| `list_scheduled_emails` | List scheduled (and recurring) emails in the workspace, soonest occurrence first. | `scheduled_emails:read` |
| `get_scheduled_email` | Fetch a scheduled email by id. | `scheduled_emails:read` |
| `create_scheduled_email` | Schedule an email to send later (optionally recurring via an RRULE). | `scheduled_emails:write` |
| `update_scheduled_email` | Update a pending scheduled email (recipient, subject, body, time, rrule). | `scheduled_emails:write` |
| `cancel_scheduled_email` | Cancel a scheduled email (soft: sets status to cancelled). | `scheduled_emails:write` |

### Scout

| Tool | Purpose | Scope |
|------|---------|-------|
| `list_scout_threads` | List the caller's Scout chat threads, newest first. | `scout:read` |
| `list_scout_messages` | List messages in a Scout thread. | `scout:read` |
| `create_scout_thread` | Start a new Scout chat thread. | `scout:write` |
| `send_scout_message` | Post a user message to a Scout thread. | `scout:write` |

Scout replies are asynchronous. `send_scout_message` returns `202` immediately; poll
`list_scout_messages(after_message_id=N)` until a message with `author_type: "ai"` and
`reply_status: "replied"` appears.

### Workflows

| Tool | Purpose | Scope |
|------|---------|-------|
| `list_workflows` | List the workspace's automation workflows. | `workflows:read` ¹ |
| `list_workflow_executions` | List a workflow's run history (newest first). | `workflows:read` ¹ |
| `trigger_workflow` | Trigger an enabled webhook workflow with an optional JSON payload. | `workflows:trigger` ¹ |

> ¹ Workflow tools appear only when `ENABLE_WORKFLOWS=1`.

## Connecting a mailbox from an agent

Before an agent can read or send email, at least one mailbox must be connected to the
workspace. There are two modes.

### Web mode (default — recommended for all deployments)

```python
connect_email_account(mode="web")
```

The tool returns a path to open in a browser. The user visits that path on their Campbooks
server, completes the normal OAuth consent flow, and the account appears in the workspace.
This is the simplest path — the server handles the redirect and token storage.

### Token mode (self-hosted only)

If your server's OAuth callback is not reachable from the public internet, you can mint a
refresh token locally using the helper script bundled with the plugin:

```bash
python3 integrations/claude-plugin/scripts/campbooks_oauth.py google \
  --client-id YOUR_GOOGLE_CLIENT_ID \
  --client-secret YOUR_GOOGLE_CLIENT_SECRET

# For Zoho, also specify your data-centre region:
python3 integrations/claude-plugin/scripts/campbooks_oauth.py zoho \
  --client-id YOUR_ZOHO_CLIENT_ID \
  --client-secret YOUR_ZOHO_CLIENT_SECRET \
  --region eu
```

Omit `--client-id` / `--client-secret` and the script will prompt for them securely. Python 3
from the standard library is all you need — no pip install. The script opens a browser,
completes consent, and prints one line of JSON to stdout:

```json
{"provider": "google", "refresh_token": "1//0e..."}
```

Pass the `refresh_token` value to `connect_email_account(mode: "token", ...)`.

> **The credentials must be the server's own OAuth client credentials** — the same
> `GOOGLE_CLIENT_ID` / `ZOHO_CLIENT_ID` the Campbooks server uses. A refresh token minted
> with a different `client_id` will fail when the server tries to refresh it.
>
> Before running the script, add `http://localhost:8765/callback` to the OAuth app's allowed
> redirect URIs (Google Cloud Console → your OAuth client → Authorised redirect URIs; Zoho
> Developer Console → your client → Redirect URIs).
>
> Keep the refresh token secure — it grants full mailbox access to anyone who holds it.

The `/campbooks:setup` skill handles the token mode path automatically when you select the
self-hosted option during onboarding.

**Microsoft 365:** web mode only. The Microsoft sign-in and mailbox connect surfaces are also
gated on `ENABLE_MICROSOFT=1` (disabled by default — see [`docs/self-hosting.md`](self-hosting.md)).

Google and Zoho support both modes.

## Self-hosted notes

- **Server URL:** point your agent at `https://<your-host>/api/mcp`. The Claude Code plugin's
  `server_url` prompt accepts any URL; it appends `/api/mcp` automatically. For every other
  client, pass the full endpoint URL.
- **Same scopes, same tools:** the MCP endpoint ships enabled on every self-hosted instance —
  no extra flag or service required. Scopes, tool families, and permission semantics are
  identical to Campbooks Cloud.
- **Feature-gated families:** `tasks:*` tools appear only when `ENABLE_TASKS=1`; `workflows:*`
  tools only when `ENABLE_WORKFLOWS=1`; `list_email_templates` only when
  `ENABLE_EMAIL_TEMPLATES=1`. All other tool families are available out of the box with no
  additional configuration.

## Safety & permissions

**Per-user permission gates still apply.** A token acts as the user who created it. The
`emails:send` scope still requires that the acting user has send permission on the chosen
account. Resources the acting user cannot see return `404`, not `403` — the API never reveals
the existence of another workspace's data.

**Rate limit:** 600 requests per minute per client (HTTP 429 when exceeded). Batch operations —
such as archiving a cluster with `skim_decide` — count as a single request.

**Confirm before write.** The `/campbooks:triage` and `/campbooks:setup` skills always name
exactly what is about to change and wait for an explicit yes before calling any write tool.
When building your own agent workflows, apply the same discipline: show the full draft before
calling `send_email`; name the cluster and email count before calling `skim_decide(archive)`;
confirm the sender before calling `set_contact_state(block)`.
