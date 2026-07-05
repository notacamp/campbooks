# Setup and accounts

## Connecting email accounts

Use connect_email_account. There are two modes:

### web mode (default)

Returns `{ connect_path: "/email_accounts/new", note: "..." }`.
Direct the user to open that path in a browser on their Campbooks server. The
full OAuth consent flow happens there; no credentials pass through this tool.
This is the right choice for cloud users and anyone who prefers the UI flow.

### token mode (self-hosted)

Accepts a `refresh_token` that was minted using the **same OAuth client
credentials configured on this Campbooks server**. This is critical: a token
from a different client_id (e.g. from the cloud production client) will fail
the refresh step with an error directing you to use the server's own client.

Process:
1. The caller obtains a refresh token using the server's OAuth client
   (client_id + client_secret from Settings → Integrations) and a redirect URI
   the provider accepts (e.g. http://localhost:8765/callback for local scripts).
2. Call connect_email_account(mode: "token", provider: "zoho"|"google",
   refresh_token: "<token>"). Optionally pass `email_address` as a check.
3. The server validates the token, resolves the account identity, creates or
   reactivates the EmailAccount, and enqueues a delta scan.

The refresh_token is never echoed back and is not logged. Pass it directly into
the tool call; do not copy it into chat messages.

Microsoft accounts require the microsoft feature to be enabled on this server
(get_setup_status.features.microsoft = true). If it is off, token mode for
Microsoft returns a ToolError.

## AI provider setup

get_setup_status.ai shows which capabilities are configured:
- `text_configured` — a provider is set for inbox categorisation, Scout chat,
  summaries, and task/reminder extraction.
- `documents_configured` — a provider is set for document field extraction.
- `managed_available` — the server's managed AI (cloud-only) is available.
- `processing_enabled` — the workspace's AI processing switch is on.

If text_configured is false, direct the user to Settings → AI on their server.
Without a configured text provider, triage suggestions and Scout chat will not
work.

## MCP key creation

Long-lived API access for agents uses MCP keys instead of short-lived
Doorkeeper tokens. To create one:
1. Go to Settings → API access → New client.
2. Choose the scopes you need (see the scope list in the plugin README).
3. After saving, the "MCP key" field shows `<uid>.<secret>` — copy it once,
   as the secret is shown only at creation time.
4. Use it as `Authorization: Bearer <uid>.<secret>` on POST /api/mcp.
   It does not expire; rotate the secret (Settings → API access → client →
   Rotate secret) or delete the client to revoke access.

Recommended scope set for a full-access agent:
emails:read emails:write emails:send tags:read tags:write
documents:read documents:write document_types:read document_types:write
contacts:read contacts:write calendar:read calendar:write
reminders:read reminders:write tasks:read tasks:write
folders:read folders:write email_accounts:read email_accounts:write
scout:read scout:write scheduled_emails:read scheduled_emails:write

For read-only or narrower agents, issue fewer scopes — tools/list will
return only the tools covered by those scopes, keeping the agent's context
smaller.
