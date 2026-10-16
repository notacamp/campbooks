# observability Specification

## Purpose
How Campbooks lets an operator see what it is doing: traces and metrics for
requests, jobs, AI calls and the database pool, exported only when the
operator asks for it, and never carrying message content, secrets or
personal data.

## Requirements

### Requirement: Nothing exported by default
The system SHALL export no traces and no metrics, and SHALL open no extra
listener, unless `CAMPBOOKS_TELEMETRY__OTLP_ENDPOINT` or
`CAMPBOOKS_TELEMETRY__METRICS_ADDR` is set. The health and version endpoints
SHALL behave the same whatever the telemetry settings.

#### Scenario: Default settings
- **WHEN** the server starts with no `CAMPBOOKS_TELEMETRY__*` setting
- **THEN** it sends nothing to any collector and listens only on `CAMPBOOKS_BIND_ADDR`

### Requirement: OTLP export
When `CAMPBOOKS_TELEMETRY__OTLP_ENDPOINT` is set, the system SHALL export
traces and metrics over OTLP/HTTP to that endpoint, with the headers of
`CAMPBOOKS_TELEMETRY__OTLP_HEADERS`, keeping the share of new traces given by
`CAMPBOOKS_TELEMETRY__SAMPLE_RATIO` (default 1.0) and honouring the sampling
decision of an incoming trace context. An endpoint that is not an http(s) URL,
or a ratio outside 0 to 1, SHALL stop the server at startup with a message
naming the setting. The headers SHALL never be logged.

#### Scenario: Bad ratio
- **WHEN** the server starts with `CAMPBOOKS_TELEMETRY__SAMPLE_RATIO=2`
- **THEN** it exits with an error naming `CAMPBOOKS_TELEMETRY__SAMPLE_RATIO`

### Requirement: Prometheus metrics endpoint
When `CAMPBOOKS_TELEMETRY__METRICS_ADDR` is set, the system SHALL serve the
metrics in the Prometheus text format at `GET /metrics` on a listener bound
to that address, separate from the API listener, and SHALL NOT serve
`/metrics` on the API listener.

#### Scenario: Scraping
- **WHEN** the metrics listener is enabled and requests have been served
- **THEN** `GET /metrics` on it answers 200 with `http_server_request_duration_seconds` series labelled by route template

#### Scenario: Not on the API
- **WHEN** a client requests `/metrics` on the API listener
- **THEN** the response is a problem+json 404

### Requirement: Request traces
The system SHALL record one server span per HTTP request named after the
method and route template, with the route template, method, status code,
request id and trace id, plus the workspace id when the route names a
workspace and the principal kind (`user` or `client`) when the caller is
authenticated. A W3C `traceparent` on the request SHALL make the span part of
that trace.

#### Scenario: Incoming trace context
- **WHEN** a request arrives with `traceparent: 00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01`
- **THEN** its span and its log lines carry trace id `4bf92f3577b34da6a3ce929d0e0e4736`

### Requirement: Job traces
The system SHALL record one consumer span per job attempt with the job's id,
kind, queue, attempt, workspace and outcome, and SHALL link it to the span
that enqueued the job (or published the event it delivers) when there was
one.

#### Scenario: A job enqueued by a request
- **WHEN** a request publishes an event and a worker later runs its delivery
- **THEN** the delivery's span has a link to the request's span

### Requirement: Outbound trace context
The system SHALL send the current trace context as a W3C `traceparent`
header on its requests to AI providers and to the identity provider, and
SHALL record a client span for each.

#### Scenario: Fetching signing keys
- **WHEN** a request with a bearer token makes the server fetch the identity provider's keys
- **THEN** the identity provider receives a `traceparent` with the request's trace id

### Requirement: Metrics
The system SHALL record request duration by method, route template and
status; job duration by kind, queue and outcome; job wait before claim; queue
depth by queue and state and the age of the oldest ready job; AI call
duration and token usage by operation, role, provider address and model;
database pool connections and acquire wait; and build information. Metric
attributes SHALL never hold ids, email addresses, paths with ids or error
messages.

#### Scenario: Route templates, not paths
- **WHEN** `GET /api/v1/w/{workspace_id}/members` is requested for two different workspaces
- **THEN** both requests count in the one series labelled `http.route=/api/v1/w/{workspace_id}/members`

### Requirement: No personal data or secrets in telemetry
Span attributes, span events, metric attributes and log fields SHALL NOT
contain email bodies or other message content, access or refresh tokens,
cookies, authorization headers, signup or invitation codes, or email
addresses. Request spans SHALL NOT record the request URI or query string.

#### Scenario: A request carrying everything
- **WHEN** a request with a bearer token, a session cookie, an email address in its query string and an email claim in its token is served, and the job it causes runs
- **THEN** none of the token, the cookie value or the email addresses appears in any exported span or any log line
