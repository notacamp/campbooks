## MODIFIED Requirements

### Requirement: Nothing exported by default
The system SHALL export no traces, no metrics and no error reports, SHALL
forward none of the app's telemetry, and SHALL open no extra listener,
unless `CAMPBOOKS_TELEMETRY__OTLP_ENDPOINT`,
`CAMPBOOKS_TELEMETRY__METRICS_ADDR`, `CAMPBOOKS_TELEMETRY__ERROR_DSN` or
`CAMPBOOKS_TELEMETRY__CLIENT_COLLECTOR_URL` is set; an empty value SHALL
count as unset. The health and version endpoints SHALL behave the same
whatever the telemetry settings.

#### Scenario: Default settings
- **WHEN** the server starts with no `CAMPBOOKS_TELEMETRY__*` setting
- **THEN** it sends nothing to any collector or error tracker and listens only on `CAMPBOOKS_BIND_ADDR`

#### Scenario: Empty settings
- **WHEN** the server starts with `CAMPBOOKS_TELEMETRY__ERROR_DSN=` and `CAMPBOOKS_TELEMETRY__CLIENT_COLLECTOR_URL=`
- **THEN** it reports no error and forwards none of the app's telemetry

### Requirement: Metrics
The system SHALL record request duration by method, route template and
status; job duration by kind, queue and outcome; job wait before claim; queue
depth by queue and state and the age of the oldest ready job; AI call
duration and token usage by operation, role, provider address and model;
database pool connections and acquire wait; connector sync runs by provider
and outcome (`ok`, `throttled`, `refused`, `failed`); the streams of the
sync schedule due more than five minutes ago; the error reports sent; the
items of the app's telemetry forwarded or dropped, by kind; and build
information. Metric attributes SHALL never hold ids, email addresses, paths
with ids or error messages.

#### Scenario: Route templates, not paths
- **WHEN** `GET /api/v1/w/{workspace_id}/members` is requested for two different workspaces
- **THEN** both requests count in the one series labelled `http.route=/api/v1/w/{workspace_id}/members`

#### Scenario: A throttled sync
- **WHEN** a Gmail sync run is throttled by the provider
- **THEN** `campbooks.connector.syncs` counts one run with `provider=google` and `outcome=throttled`, and no attribute names the connection, stream or workspace

## ADDED Requirements

### Requirement: Error reports
When `CAMPBOOKS_TELEMETRY__ERROR_DSN` is set, the system SHALL send one
report to that Sentry-compatible error tracker for each internal error, each
job that failed for good and each panic, tagged with the release (the
build's `GIT_SHA`, else the server's version) and the environment
(`CAMPBOOKS_TELEMETRY__ENVIRONMENT`, default `production`), and SHALL flush
queued reports when it shuts down. Nothing below the ERROR level SHALL be
reported, and no breadcrumbs, sessions or traces SHALL be sent. A DSN that
does not parse, or an environment other than letters, digits, `-`, `_` and
`.`, SHALL stop the server at startup with a message naming the setting and
never the DSN.

#### Scenario: An internal error
- **WHEN** a request to `/api/v1/w/{workspace_id}/messages` fails with an internal error while a DSN is set
- **THEN** one report reaches the tracker with the request's id, trace id and workspace id, the route template as its culprit, the release and the environment

#### Scenario: A bad DSN
- **WHEN** the server starts with `CAMPBOOKS_TELEMETRY__ERROR_DSN=not a dsn`
- **THEN** it exits with an error naming `CAMPBOOKS_TELEMETRY__ERROR_DSN` that does not contain the value

#### Scenario: A warning
- **WHEN** a sync run fails and will retry (a warning)
- **THEN** no report is sent

### Requirement: Error reports carry ids and error kinds only
An error report SHALL be built from an allowlist and SHALL contain only: its
id, level, time, release, environment, the server's role and version, an
exception whose type and value are error kinds, the ids, route template,
method, status, job kind, queue and attempt of the request or job it
happened in, and the place it was logged. An error kind SHALL be the
message's text before its first `: `, `; ` or ` (`, with quoted text,
email addresses, the path and query of URLs and token-like runs masked, at
most 120 characters; a panic's SHALL be where it happened, never its
message. A tag SHALL be sent only when its value looks like an id or a name.
Every report SHALL pass, before it is sent, a scrubber that rebuilds it from
the same allowlist, so that no user, request, cookie, header, request body,
context, extra data, breadcrumb, server name or module list leaves the
process whatever produced the report. Reports SHALL be sent with
`send_default_pii` off.

#### Scenario: An error quoting a row
- **WHEN** a job fails for good with `"Q4 restructuring plan" from maya@example.com: <the email's body>`
- **THEN** the report's exception value is `"…" from [address]`, and neither the subject, the address nor the body appears anywhere in it

#### Scenario: A report carrying everything
- **WHEN** a report reaches the scrubber with a user's address and IP, a request with an `Authorization` header, a session cookie, a query string and a body, a breadcrumb naming an attachment, extra data with a body and a context with an access token
- **THEN** the report sent holds none of them, and keeps its request id, workspace id and route template

#### Scenario: A panic
- **WHEN** a job handler panics with a message quoting a search query and an address
- **THEN** the report's exception is `panic` with the file and line it happened at, and holds neither the query nor the address

### Requirement: Test error report
`campbooks-server telemetry test-error` SHALL send one report through the
production pipeline as if a request had failed, with invented fields that
include an email address and a token, print the report's id, and exit
non-zero when no DSN is set or the report could not be queued.

#### Scenario: Checking a tracker
- **WHEN** an operator runs `campbooks-server telemetry test-error` with a DSN set
- **THEN** a report with a request id, a nil workspace id and the route template `/api/v1/system/telemetry-test` reaches the tracker, without the invented address or token, and its id is printed

### Requirement: The app's telemetry relay
`POST /api/v1/system/client-telemetry` SHALL accept a batch of the app's own
exceptions, measurements and events from a caller holding `system:read`
through a token not bound to a workspace, and SHALL answer 404 when
`CAMPBOOKS_TELEMETRY__CLIENT_COLLECTOR_URL` is unset. Otherwise it SHALL
answer 202 and forward the scrubbed batch, in Grafana Faro's format with
the app named `campbooks` and the configured environment, to that URL, with
`CAMPBOOKS_TELEMETRY__CLIENT_COLLECTOR_KEY` as `x-api-key` when set. The
key and the batch SHALL never be logged; nothing SHALL be stored.

#### Scenario: No collector
- **WHEN** the app posts a batch and no collector URL is set
- **THEN** the answer is 404 and nothing is forwarded

#### Scenario: Forwarding
- **WHEN** the app posts a batch with one exception and a collector URL and key are set
- **THEN** the answer is 202 and the collector receives one Faro payload with that exception and the `x-api-key` header

#### Scenario: No token
- **WHEN** a batch is posted without an access token
- **THEN** the answer is 401 and nothing is forwarded

### Requirement: The app's telemetry carries no personal data
The relay SHALL rebuild each batch from the fields it defines, never reading
others, and SHALL forward at most 20 exceptions, 20 measurements and 20
events, and at most 30 frames per exception. Exception types and messages
SHALL be forwarded as error kinds; a frame's script as its file name only
and its function only when it looks like a name; a measurement only when its
kind looks like a name and with only the finite values `cls`, `fcp`, `fid`,
`inp`, `lcp`, `ttfb` and `delta`; an event only when its name looks like a
name; the session id only when it looks like an id; the view only when it is
a route template whose segments are `$parameters` or lowercase letters, `-`
and `_`; browser fields only as short names.

#### Scenario: A batch carrying everything
- **WHEN** a batch names a page `/w/<workspace id>/search?q=<subject>`, an exception whose message quotes a subject and an address and holds a token, a frame whose script URL carries a token, and a measurement keyed by an address
- **THEN** the forwarded payload holds none of the subject, the address, the token or the workspace id

#### Scenario: A route template
- **WHEN** a batch names the view `/w/$workspaceId/inbox`
- **THEN** the forwarded payload's view is `/w/$workspaceId/inbox`

### Requirement: The app reports its own errors and timings
The app SHALL report the errors it did not handle (uncaught errors,
unhandled rejections and errors a route's error screen caught) and its web
vitals to the relay, in batches sent at most every five seconds and when the
page is hidden, with the route template shown, a random id for this run of
the app and the browser's name and platform where the browser states them;
never the page's address. A thrown value that is not an Error SHALL be
reported by its type only. A failed send SHALL be dropped, never retried; a
404 SHALL stop reporting until the app reloads; an ended web session SHALL
NOT send the browser to sign in.

#### Scenario: The instance collects nothing
- **WHEN** the relay answers 404
- **THEN** the app sends no further batch until it reloads

#### Scenario: A thrown object
- **WHEN** code throws `{ subject: "Q4 restructuring plan" }`
- **THEN** the reported exception's kind is `object` and the subject is not sent
