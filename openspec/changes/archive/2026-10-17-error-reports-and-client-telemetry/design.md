# Design

## Server error reports

`error_reports::init` starts a `sentry` client when
`CAMPBOOKS_TELEMETRY__ERROR_DSN` holds a valid DSN (an invalid one stops the
server, naming the setting, never the value). Options: `send_default_pii`
off, `default_integrations` off (no panic integration: the server's own
hook already withholds panic messages), no breadcrumbs, no sessions, no
traces, release = `GIT_SHA` (else `campbooks-server@<version>`),
environment = `CAMPBOOKS_TELEMETRY__ENVIRONMENT` (default `production`).
`Telemetry::shutdown` drops the guard, which flushes for up to 5 s.

The `sentry-tracing` integration is not used: it copies every field of
every enclosing span into the event, so its safety would rest on the
scrubber alone. `ErrorReportLayer`, a `tracing` layer installed only when a
DSN is set, builds the report itself:

- On every span it keeps only the allowlisted fields (`request_id`,
  `trace_id`, `workspace.id`, `principal.kind`, `http.route`,
  `http.request.method`, `http.response.status_code`, `error.type`,
  `job.id`, `job.kind`, `job.queue`, `job.attempt`).
- On an ERROR event it reads the message, the `error` field and a few
  allowlisted event fields (`error.type`, `job_id`, `kind`, `operation`,
  `kid`, `locale`), and nothing else. The report is one exception whose
  type is the message (or `panic`) and whose value is the `error` field,
  both as `redact::error_kind`; tags are the allowlisted fields; culprit
  and transaction are the route template or the job kind; the fingerprint
  is type + culprit + location (`crates/…/file.rs:line`).

`error_reports::scrub`, the client's `before_send`, rebuilds every event
from scratch: id, level, timestamp, platform, release, environment, SDK;
logger, culprit and transaction only if they look like ids; tags only if
the key is allowlisted and the value looks like an id; message, exception
types and values, and fingerprint parts as error kinds. User, request,
contexts, extra, breadcrumbs, server name, modules, threads and stack traces
are never copied. It then adds the role and version tags and counts the
report in `campbooks.errors.reported`.

`kernel::redact` gains two helpers both paths use:

- `error_kind(text)`: `job_error`'s redaction (quotes, addresses, URL paths
  and queries, opaque runs, extra lines masked), then the text before the
  first `: `, `; ` or ` (` (where messages go on to quote what they failed
  on), at most 120 characters.
- `looks_like_an_id(value)`: at most 200 characters of letters, digits and
  `-_.:/{}`, with no run of 40 or more between `/`, `{`, `}` and `:` (a
  token).

Stack traces are not attached: release builds are stripped (`strip =
"symbols"`), so frames would be bare addresses.

## The app's telemetry

The app cannot reach a collector itself (`connect-src 'self'` on the web,
only its bridge natively) and must not hold the collector's key, so it
posts to its own server: `POST /api/v1/system/client-telemetry`, in the
`system` module (`client_telemetry` slice). Scope `system:read`, which the
web BFF and the native app already ask for at sign-in (so existing sessions
work), through `Unbound` (a workspace-bound token gets 404); access
`system.client_report` (`Tenancy::Account`, owner `Create` only); counted
in the write rate-limit class like every scoped mutation. No data is
stored; no event is published.

The request type (`ClientReport`) is the app's own, not Faro's: a session
id, platform, app version, view (route template), browser name/version/OS,
and lists of exceptions (kind, message, timestamp, frames: file, function,
line, column), measurements (kind, values) and events (name). Unknown
fields are never read. The server:

- answers 404 when no collector URL is set, so the app stops;
- caps each list at 20 items and stacks at 30 frames;
- keeps a session id or event name only if it looks like an id, a
  measurement only if its kind does and with only finite `cls`, `fcp`,
  `fid`, `inp`, `lcp`, `ttfb`, `delta` values;
- keeps a view only if every segment is a `$parameter` or lowercase
  letters, `-` and `_` (no digits: no id passes);
- keeps a frame's file as its file name (no host, path or query) and its
  function only if it looks like an id;
- turns exception kinds and messages into error kinds;
- keeps browser fields only as short names;
- answers 202 and forwards the Faro payload (`meta.app.name = campbooks`,
  `meta.app.environment` from the setting) in the background, 5 s timeout,
  `x-api-key` when a key is set; counts items in
  `campbooks.client_telemetry.items` (`forwarded` / `dropped`). The key and
  the payload are never logged.

In the app, `lib/telemetry` (started once from `app/providers.tsx`) listens
to `error` and `unhandledrejection`, the router's `defaultOnCatch`, and
`web-vitals`; batches up to 20 items for 5 s or until the page is hidden;
sends through `backgroundApi`, an API client whose web transport never
sends the browser to sign in; drops a failed batch; stops for good on 404.
A thrown non-Error sends only its `typeof`. The view is the last route
match's `routeId`.

## Metrics

- `campbooks.connector.syncs` (counter; `provider`, `outcome` =
  `ok` | `throttled` | `refused` | `failed`), recorded where a run's outcome
  is decided in `connectors.sync_stream`.
- `campbooks.connector.overdue_streams` (gauge), the schedule rows due more
  than 5 minutes ago, sampled by the dispatcher each tick in its
  maintenance transaction (ids and times only, as before).
- `campbooks.errors.reported` (counter; `source` = `server`).
- `campbooks.client_telemetry.items` (counter; `kind`, `outcome`).

## Data ownership and events

No schema change; no event published or consumed.
