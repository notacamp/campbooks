# System health

Campbooks talks to a lot of outside services: mail providers (Gmail, Zoho Mail, Microsoft 365), calendars, Google Drive, Notion, AI providers, workflow webhooks, push gateways, and SMTP. **System health** records every one of those calls and shows which connections are working and which are failing — per workspace, and instance-wide.

## The workspace view

**Settings → System health** (visible to workspace admins) shows the workspace's own slice:

- A summary of the workspace's calls and errors over the last 24 hours.
- One card per service the workspace actually used: its state, an hourly activity sparkline with errors highlighted, call volume, error rate, average response time, and the most recent error. Clicking a card filters the log to that service.
- A call log of the workspace's calls, filterable by service, outcome, and time window (24 hours, 7 or 30 days).

Calls are attributed to a workspace through the account that made them (its mailboxes, calendars, AI runs, workflow actions) — account-bound clients carry their workspace directly, so attribution holds even for calls triggered outside a job or request (a console session, a maintenance task). Calls that belong to no workspace — transactional email, for example — appear only in the instance view.

## The instance view

`/admin/system_health` (visible to app admins only) is the sum of all workspaces plus the instance-level calls. Same layout, with two additions: services with no activity are listed so a silent integration is visible, and log rows name the workspace they belong to.

States are derived from the last 24 hours of calls:

| State | Meaning |
| --- | --- |
| Operational | Calls are succeeding (error rate below 5%). |
| Degraded | Error rate is 5% or higher, with at least 3 errors. |
| Failing | Errors with no recent successes, or an error rate of 50%+ across 5+ errors. |
| Idle | No calls in the window. |

Expected protocol responses (for example Google Calendar's 410 "sync token expired", which just triggers a full resync) are counted as successes so routine control flow never looks like an outage.

## What is recorded

One row per call: the service, a normalized operation (HTTP method plus the path with identifiers replaced by `:id`), success or error, the HTTP status, the duration, the error class, a sanitized error message, and the full request and response headers and bodies (sanitized and capped). For AI provider calls, the model name and token counts are also stored in the row's metadata.

**Security and privacy:** captured bodies and headers are visible **only in the instance admin view** (`/admin/system_health` → call detail page). The workspace-facing System health view (Settings → System health) shows metadata only — no bodies, no headers — because captured content can include mailbox data that per-user permissions would otherwise protect.

Credential safety is enforced before any data reaches the database:

- The following request headers are dropped entirely (not even stored as `[REDACTED]`): `Authorization`, `Proxy-Authorization`, `Cookie`, `Set-Cookie`, `X-Api-Key`, `Api-Key`, `X-Auth-Token`, `X-Goog-Api-Key`.
- Body credential fields are redacted: values of keys matching `api_key`, `access_token`, `refresh_token`, `client_secret`, `id_token`, `password`, `secret`, and `token` in JSON bodies become `"[FILTERED]"`. URL query strings, `Bearer` tokens, `key=value` pairs, and `sk-…` API-key strings are also redacted.
- Bodies are capped at 10 KB; binary or non-UTF-8 payloads are stored as a placeholder (e.g. `[binary image/png, 45231 bytes]`).
- Redaction always happens before truncation, so a secret never straddles the cut point.

What is deliberately **not** recorded in any view: recipient email addresses and anything that identifies individuals.

## Retention

The daily retention sweep prunes successful calls after 30 days and errors after 90 days. No configuration is required.

## Disabling it

Set `DISABLE_SYSTEM_HEALTH=1` to stop recording entirely. The dashboard stays reachable but will show no new data.

## Instrumenting a new client

Faraday-based clients add one line, first in the connection builder:

```ruby
Faraday.new do |f|
  f.use SystemHealth::FaradayMiddleware, service: "my_service"
  # optionally: expected_statuses: [410] for statuses that are protocol
  # control flow rather than failures
  ...
end
```

Anything else wraps the call:

```ruby
SystemHealth.track(service: "my_service", operation: "sync") do
  client.do_the_thing
end
```

Add the service key to `SystemHealth::SERVICES` (it groups the dashboard) and a display name under `system_health.services.*` in the locale files. Unknown services still record and appear under "Other".
