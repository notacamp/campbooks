# Proposal

## Why

The server writes logs and, when asked, metrics, but nothing tells an
operator that it is failing: an internal error, a job that failed for good
or a panic is one log line among thousands, and the app's own errors and
how fast its pages load never leave the browser. An operator who runs a
Sentry-compatible tracker (GlitchTip, Sentry) or a Grafana Faro receiver
should be able to point Campbooks at it with one setting each. Campbooks
handles people's mail, so what reaches those services must be ids and
error kinds, never a body, subject, address, attachment name, token,
cookie, header or request body.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/186.
The issue asked for this through the cloud extension point; a DSN setting
is the same kind of operator opt-in as the OTLP endpoint, holds nothing
private, and lets a self-hoster use their own tracker, so it lives here.

## What Changes

- **Server error reports** (`CAMPBOOKS_TELEMETRY__ERROR_DSN`, off when
  unset): every ERROR event (an internal error, a job failed for good, a
  panic) becomes one report, built from an allowlist: the error's kind (its
  message's head, redacted), the ids of the request or job it happened in
  (request, trace, workspace, job), the route template or job kind, the
  place it was logged. A `before_send` scrubber rebuilds every outgoing
  report from the same allowlist again. No PII, no breadcrumbs, no
  integrations, no traces. Tagged with the release (`GIT_SHA`) and the
  environment (`CAMPBOOKS_TELEMETRY__ENVIRONMENT`, default `production`).
- **`campbooks-server telemetry test-error`** sends one report through the
  production pipeline, with an invented address and token the scrubber
  must drop, so an operator sees what a real report holds.
- **The app's own telemetry**: the app reports the errors it did not handle
  (window errors, unhandled rejections, errors a route's error screen
  caught) and its web vitals, in batches, to
  `POST /api/v1/system/client-telemetry` (`system:read`, which every app
  already holds). The server scrubs each report and forwards it to the Faro
  receiver at `CAMPBOOKS_TELEMETRY__CLIENT_COLLECTOR_URL` (with
  `__CLIENT_COLLECTOR_KEY` as `x-api-key`); without one it answers 404 and
  the app stops sending. The app's pages keep `connect-src 'self'`.
- **Metrics**: connector sync runs by provider and outcome, streams
  overdue in the sync schedule, error reports sent, client telemetry items
  forwarded or dropped.
- Docs: `deploy/README.md` (Monitoring), `deploy/.env.example`, CHANGELOG.

## Capabilities

### Modified Capabilities

- `observability`: error reports, the app's telemetry relay, what both may
  contain, and the new metrics.

## Impact

- Modules touched: `system` (the `client_telemetry` slice and the
  `system.client_report` resource), `connectors` (sync metrics); the
  kernel's `config` and `redact`; the server's `error_reports` and CLI;
  the client's `lib/telemetry` and `lib/api` (a transport that never
  navigates). No new module, no cross-module dependency, no migration.
- New dependencies: `sentry` (server, no default features: reqwest and
  rustls only) and `web-vitals` (client). The Grafana Faro web SDK is not
  used: its `ua-parser-js@1.0.41` fails the client's provenance policy; the
  app speaks Faro's model through the server instead.

## Non-goals

- Traces or session replays in the error tracker.
- Symbolicated stack traces: release builds are stripped, so reports name
  the place an error was logged instead.
- Dashboards and alert rules (the operator's, kept with their monitoring).
- Reporting from the sign-in pages served by the identity provider.
