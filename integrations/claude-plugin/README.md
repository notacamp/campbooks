# Campbooks — Claude Code plugin

Connect Claude Code agents to your Campbooks inbox over MCP. The plugin exposes
`/api/mcp` as a Model Context Protocol server so agents can triage email, review
documents, manage tasks and calendar events, and run the skim loop — all in plain
language, without writing a line of code.

Full reference: <https://campbooks.not-a-camp.com/docs/ai-agents/overview>

---

## Install

```
/plugin marketplace add notacamp/campbooks
/plugin install campbooks@campbooks
```

Claude Code will prompt for:
- **Campbooks server URL** — `https://app.campbooks.not-a-camp.com` for Campbooks Cloud,
  or your self-hosted URL (no trailing slash).
- **MCP key** — see below.

---

## Creating the MCP key

The MCP key is a non-expiring credential for agent sessions. Short-lived tokens (2 h)
do not work in static agent configs; the MCP key solves that.

1. In your Campbooks instance open **Settings → API access** and click **New client**.
2. Name it (e.g. "Claude Code agent") and select the scopes it needs (see Scopes below).
3. Click **Create**. On the next screen you will see the **MCP key** — a single string
   in the form `<client-id>.<client-secret>`. Copy it now; it is shown only once.
   If you lose it, regenerate the client secret from the same Settings page.
4. Paste the key into the `mcp_key` field when configuring the plugin.

Rotate the key by regenerating the client secret in Settings → API access. Revoking
individual access tokens does not invalidate an MCP key; deleting the client or
regenerating its secret does.

---

## Scopes

Scopes control which MCP tools appear in `tools/list`. Narrower scope = fewer tools =
less context used per agent session.

Recommended full set:

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

Minimal read-only set for a query-only client:

```
emails:read documents:read calendar:read tasks:read
```

The three meta tools (`get_overview`, `get_setup_status`, `guide`) require no scope and
appear for any authenticated client.

---

## Skills

The plugin ships two skills you can invoke directly:

### `/campbooks:setup`

Guided onboarding: creates the API client, connects a mailbox, configures AI parsing,
bootstraps document types and tags, and walks through the first skim session.

```
/campbooks:setup
```

### `/campbooks:triage`

Daily inbox run: overview → skim deck → awaiting-reply → pending documents → suggested
tasks and reminders → closing summary.

```
/campbooks:triage
```

---

## Self-hosted server URL

Set `server_url` to your instance's public URL (e.g. `https://campbooks.example.com`).
No trailing slash. The plugin appends `/api/mcp` automatically.

---

## Local OAuth (self-hosted only)

If your server's OAuth callbacks are not accessible from the public internet, you can
mint a refresh token locally and hand it to the server via the MCP API.

> **This is for self-hosted operators only.** Campbooks Cloud users should use the
> `connect_email_account(mode: "web")` flow instead — the web OAuth flow is simpler
> and the server handles the redirect.

### Prerequisites

- Python 3 (standard library only — no pip install needed).
- The **server's own** OAuth client credentials: `GOOGLE_CLIENT_ID`/`GOOGLE_CLIENT_SECRET`
  or `ZOHO_CLIENT_ID`/`ZOHO_CLIENT_SECRET` from the server's environment. Tokens minted
  with a different `client_id` will fail when the server tries to refresh them.
- `http://localhost:8765/callback` added to the OAuth app's allowed redirect URIs
  (Google Cloud Console → Credentials → your OAuth client → Authorized redirect URIs;
  Zoho Developer Console → API Console → your client → Redirect URIs).

### Run

```bash
python3 integrations/claude-plugin/scripts/campbooks_oauth.py google \
  --client-id YOUR_GOOGLE_CLIENT_ID \
  --client-secret YOUR_GOOGLE_CLIENT_SECRET

# Zoho — specify your server's data-centre region
python3 integrations/claude-plugin/scripts/campbooks_oauth.py zoho \
  --client-id YOUR_ZOHO_CLIENT_ID \
  --client-secret YOUR_ZOHO_CLIENT_SECRET \
  --region eu
```

If you omit `--client-id` / `--client-secret`, the script prompts for them securely.

The script opens your browser, completes the consent flow, and prints one line of JSON
to stdout:

```json
{"provider": "google", "refresh_token": "1//0e..."}
```

Pass the `refresh_token` value to `connect_email_account(mode: "token", ...)` — the
`/campbooks:setup` skill handles this automatically when you choose the self-hosted path.

Keep the refresh token secure. It grants full mailbox access.

---

## Other agents

The MCP endpoint is plain streamable HTTP with a Bearer credential — any MCP-capable
client works. Replace `YOUR_MCP_KEY_HERE` / `$CAMPBOOKS_MCP_KEY` with your MCP key or
an environment variable that holds it.

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

For self-hosted instances, replace `https://app.campbooks.not-a-camp.com` with your
server URL in every snippet above.

---

## Links

- Full MCP reference and agent guides: <https://campbooks.not-a-camp.com/docs/ai-agents/overview>
- Campbooks REST API: `docs/api.md` in the repo, or the hosted browsable reference.
- Self-hosting guide: `docs/self-hosting.md`.
