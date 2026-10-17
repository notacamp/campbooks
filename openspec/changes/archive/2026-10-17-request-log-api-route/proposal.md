# Proposal

## Why

The server writes one line per request when it ends, and records request
durations by route template. The web client calls the API through the BFF
(`/bff/api/*`), which runs each call through the API router in-process,
without the request middleware. So every web call is logged, traced and
counted under the BFF's catch-all route `/bff/api/{*path}`: an operator
reading the request log, or the request rate and latency by route, cannot
tell a thread list from a search from a sign-out. In production almost all
app traffic is web traffic.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/186
(monitoring the Next server).

## What Changes

- A BFF request's span, log line and request metrics are named after the
  API route template it ran through (`GET /api/v1/w/{workspace_id}/members`),
  and its span carries `campbooks.via=bff`. A call that matches no API route
  keeps the BFF's own route.
- The request log line becomes a requirement: one line per request with the
  method, route template, status, duration and the request's ids, never its
  path or query string.

## Impact

- `crates/server`: `telemetry::served_route` (a route layer of the BFF's
  in-process routers) and `observe`; the BFF installs the layer.
- Metrics: series for web traffic move from `http.route=/bff/api/{*path}` to
  the API's route templates (same cardinality as direct API calls).
- No new setting, no API change, nothing more is logged.
