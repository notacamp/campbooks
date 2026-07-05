---
name: setup
description: >
  Use this skill to set up Campbooks: create the MCP API client, connect a mailbox, configure
  AI parsing, bootstrap document types and tags, and walk through the first skim session.
  Invoke when the user says "set up Campbooks", "connect my inbox", "onboard me",
  "help me get started", or reports the plugin is not connecting or needs configuration.
---

You are guiding the user through Campbooks setup, one step at a time. Ask one question,
wait for the answer, then proceed. Skip any step the user asks to skip.

**Rules for this entire skill:**
- Never create tags, document types, folders, or connected accounts without the user's
  explicit "yes".
- Never send, compose, or draft email.
- Never block a sender or trash emails without naming the specific sender and getting a yes.
- Keep each exchange to a single question.

---

## Step 0 — Verify the MCP connection

Call `get_setup_status`. If it succeeds, continue to Step 1.

If the call fails (unreachable or auth error), walk the user through getting a working key:

**a) Create the API client**

Open Settings → API access at `<server_url>/settings/api_clients` and click **New client**.
Give it a name (e.g. "Claude Code agent"). Select scopes — the full recommended set
(copy-paste this block):

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

Narrower scope sets are fine — fewer scopes mean fewer tools in `tools/list`, which uses
less context per session. Adjust to what you actually need.

Click **Create**. On the next page you will see the **MCP key** — a single string in the
form `<client-id>.<client-secret>`. Copy it now. It is shown only once; regenerate the
client secret if you lose it.

**b) Configure the plugin**

In Claude Code run `/plugin` → campbooks and set:
- `server_url`: `https://app.campbooks.not-a-camp.com`, or your self-hosted URL (no trailing slash)
- `mcp_key`: the MCP key you just copied

Then retry `get_setup_status`.

---

## Step 1 — Connect a mailbox

Check `setup_status.email_accounts`. If `count == 0` or no accounts are active, ask:
"Are you on Campbooks Cloud or running it yourself?"

**Cloud / simple path:**
Call `connect_email_account(mode: "web")`. Tell the user:
"Open this path on your Campbooks server in a browser to connect your mailbox:
`<connect_path>`"
(Use only the path the tool returns — do not guess the host.)

Wait for them to confirm they completed it, then call `get_setup_status` again to confirm
the account appears.

**Self-hosted / advanced path:**
Offer the local OAuth script. Explain the prerequisites first:
- `--client-id` and `--client-secret` must be the **server's** own OAuth client credentials
  (the same `GOOGLE_CLIENT_ID` / `ZOHO_CLIENT_ID` the Campbooks server uses).
- Add `http://localhost:8765/callback` to the OAuth app's allowed redirect URIs in
  Google Cloud Console or the Zoho developer console.
- A refresh token minted with a different `client_id` will fail when the server tries to
  refresh it.

Then instruct them to run:
```
python3 ${CLAUDE_PLUGIN_ROOT}/scripts/campbooks_oauth.py google
```
or for Zoho (with the appropriate region):
```
python3 ${CLAUDE_PLUGIN_ROOT}/scripts/campbooks_oauth.py zoho --region eu
```

The script will open their browser and, on success, print a single line of JSON to
stdout. Tell them: "Keep the refresh token secure — it grants full mailbox access to
anyone who has it."

When they confirm the script printed JSON, call `connect_email_account` with the token
directly — do not ask them to paste the refresh token into chat:
```
connect_email_account(mode: "token", provider: "<provider>", refresh_token: <token from script>)
```

After connecting, confirm the account appeared with `get_setup_status` and report the
email address.

---

## Step 2 — AI parsing

Report what `setup_status.ai` shows:
- Both `text_configured` and `documents_configured` are true → AI is ready, nothing to do.
- `managed_available` is true → ask: "Campbooks has a managed EU-default AI available.
  Would you like to enable it, or bring your own API keys?"
- Neither → direct them to Settings → AI on their server.

Explain briefly what AI parsing enables once configured: email categorization and
summaries, and automatic reminder/task extraction from emails and attachments.

If the user wants to skip AI configuration for now, accept it and move on.

---

## Step 3 — Taxonomy

Check `setup_status.taxonomy`. If all counts are 0 (no document types, tags, or folders),
ask: "What kind of work does this inbox handle?" (freelance client work, small business,
personal + side projects, etc.)

Based on their answer, propose a starter set — for example, for a freelance consultant:
- Document types: "Invoice / Billing", "Contract / Legal", "Receipt / Expenses"
- Tags: "urgent", "client", "follow-up"

Show the full proposed list at once, then ask: "Shall I create these, or would you like
to adjust them first?"

Only call `create_document_type`, `create_tag`, or `create_folder` after they confirm.
If they want changes, take the revised list and confirm once more before creating.

---

## Step 4 — First skim session

Call `get_skim_deck`. Show a brief summary: how many rings, how many clusters, and a
couple of example cluster titles to give the user a sense of what's there.

Pick the top 1–2 clusters from the priority or scout-suggestion ring and propose a decision:
"The [ring] ring has [N] clusters. The top one — '[title]' — looks like [summary].
Would you like to archive these [count] emails, keep them, or decide cluster by cluster?"

After the user answers, call `skim_decide` with their choice and confirm the result.

Explain the rhythm briefly: checking the skim deck each morning takes 2–3 minutes.
Clusters group emails by sender or topic so you decide once per cluster, not per message.
Awaiting-reply threads surface emails where you sent the last message and haven't
heard back — Campbooks tracks those automatically.

---

## Step 5 — Done

Summarize what was set up: accounts connected, AI status, taxonomy created, skim decisions
made. Then:

"Your inbox is ready. Run `/campbooks:triage` any time to work through what's waiting."
