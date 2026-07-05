# System health

Campbooks talks to a lot of outside services: mail providers (Gmail, Zoho Mail, Microsoft 365), calendars, Google Drive, Notion, AI providers, workflow webhooks, push gateways, and SMTP. **System health** records every one of those calls and gives instance admins a dashboard to see which connections are working and which are failing.

## The dashboard

`/admin/system_health` (visible to app admins only) shows:

- A summary of calls and errors over the last 24 hours.
- One card per active service: its state, an hourly activity sparkline with errors highlighted, call volume, error rate, average response time, and the most recent error. Clicking a card filters the log to that service.
- A call log, filterable by service, outcome, and time window (24 hours, 7 or 30 days).

States are derived from the last 24 hours of calls:

| State | Meaning |
| --- | --- |
| Operational | Calls are succeeding (error rate below 5%). |
| Degraded | Error rate is 5% or higher, with at least 3 errors. |
| Failing | Errors with no recent successes, or an error rate of 50%+ across 5+ errors. |
| Idle | No calls in the window. |

Expected protocol responses (for example Google Calendar's 410 "sync token expired", which just triggers a full resync) are counted as successes so routine control flow never looks like an outage.

## What is recorded

One row per call: the service, a normalized operation (HTTP method plus the path with identifiers replaced by `:id`), success or error, the HTTP status, the duration, the error class, and a sanitized error message. Workspace context is attached when known.

What is deliberately **not** recorded: request or response bodies, headers, email addresses or recipients, and anything credential-shaped. Query strings are stripped from URLs, and `Bearer` tokens, `key=value` credential pairs, and API-key-shaped strings are redacted from error messages before storage.

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
