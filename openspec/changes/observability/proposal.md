# Proposal

## Why

The server already writes JSON logs with a request id, but nothing tells an
operator how fast requests are, how deep the job queues run, how slow the AI
providers answer or how close the database pool is to exhaustion, and a log
line cannot be followed from a request into the jobs it enqueued or the
provider calls they made. The hosted cloud needs traces and metrics in its
collector; a self-hoster needs nothing to run by default and a plain metrics
page when they want one. Both must never see a message body, a token or an
email address.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/30.

## What Changes

- **Traces.** One span per HTTP request (route template, method, status,
  request id, workspace id when the route has one, principal kind), per job
  attempt (kind, queue, attempt, workspace, outcome), per AI call (operation,
  role, provider address, model, token counts, error type) and per identity
  provider fetch. Database pool waits are span events. Incoming W3C
  `traceparent`/`tracestate` is honoured; outbound calls to AI providers and
  the identity provider carry it. A job links to the trace that enqueued it
  (new nullable column `kernel.jobs.trace_context`).
- **Logs.** Request and job spans carry `trace_id`, so every JSON log line
  inside them can be joined to its trace. The request span no longer logs the
  full URI (query strings can carry personal data); it logs the route
  template.
- **Metrics** (OpenTelemetry instruments, low-cardinality attributes only):
  HTTP request duration (RED), job duration by outcome, job wait before
  claim, queue depth and oldest ready age per queue, AI call duration and
  token usage by operation, role, provider and model, database pool
  connections and acquire wait, build info.
- **Export, off by default.**
  `CAMPBOOKS_TELEMETRY__OTLP_ENDPOINT` turns on OTLP/HTTP export of traces
  and metrics (`__OTLP_HEADERS` secret, `__SAMPLE_RATIO`).
  `CAMPBOOKS_TELEMETRY__METRICS_ADDR` serves Prometheus text at `/metrics`
  on a separate listener, documented as not for the internet. Health and
  version endpoints are unchanged.
- **Privacy test.** A representative request (bearer token, cookie, email in
  the query, email claim) and the job it causes are run with the production
  log and trace pipeline; the test fails if a token, cookie or email address
  appears in a log line or span attribute.
- Docs: `deploy/README.md` (Monitoring), `deploy/.env.example`, CHANGELOG.

## Capabilities

### New Capabilities

- `observability`: traces, metrics and their export; what telemetry may and
  may not contain.

### Modified Capabilities

- `events-and-jobs` gains no new behaviour; a job row records the trace
  context of whoever enqueued it.

## Impact

- Crates: `campbooks-kernel` (new `telemetry` module, `TelemetryConfig`,
  `AppState.metrics`, spans and metrics in the worker, trace context on
  enqueue, recorded fields in `Scoped`/`Workspace`, IdP fetch span),
  `campbooks-ai` (spans and metrics around every call, `traceparent` on
  outbound requests), `campbooks-server` (subscriber, SDK and exporters,
  request middleware, `/metrics` listener, pool instruments). No domain
  module changes, no new module, no `architecture.toml` entry.
- Data: schema `kernel`, migration `20261013180000_kernel__job_trace_context.sql`
  (one nullable column).
- Dependencies (Apache-2.0 unless noted): `opentelemetry` 0.33.0,
  `opentelemetry_sdk` 0.33.0, `opentelemetry-otlp` 0.33.0 (HTTP/protobuf,
  blocking reqwest 0.13 already in the graph; no gRPC),
  `opentelemetry-prometheus` 0.33.0 + `prometheus` 0.14.0 (no default
  features), `tracing-opentelemetry` 0.34.0 (MIT), `log` 0.4 (already in the
  graph, to set sqlx's acquire event level). New transitive crates: `prost`,
  `rand` 0.9 and their small dependencies.
- Self-hosters: nothing changes unless they set `CAMPBOOKS_TELEMETRY__*`.
  Log lines inside a request or job gain `trace_id`; the request span logs
  `http.route` instead of `uri`.
- OpenAPI: unchanged. `/metrics` is served on the operator listener, not the
  API.

## Non-goals

- **Error reporting** to a Sentry-compatible service (GlitchTip): a separate
  cloud-only hook, tracked on the issue.
- **Logs over OTLP.** Logs stay on stdout as JSON for the container runtime
  to ship; they carry `trace_id` to join them to traces.
- **Dashboards and alert rules**: the cloud repo owns them.
- **Per-workspace or per-user metrics.** Ids never become metric labels.
- **Tracing every SQL statement.** sqlx's statement logging stays as it is;
  pool waits are recorded, queries are not spans.
- **Authentication on `/metrics`.** It is bound to an address the operator
  chooses and is not published by the compose file.
