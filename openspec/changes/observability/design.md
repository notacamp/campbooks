# Design

## Context

Every request and job already runs in a `tracing` span (`tower-http`'s
`TraceLayer` with a request id; the worker's `job` span), logs are JSON on
stdout, and the binary runs as `api`, `worker` or `all` (ADR 0011). The
self-host default is one container plus Postgres; the hosted cloud runs each
role as replicas and has a telemetry collector. Nothing measures anything yet.

## Data ownership and events

- **Schema `kernel`**: migration `20261013220000_kernel__job_trace_context.sql` adds
  `kernel.jobs.trace_context text NULL`, the W3C `traceparent` of the span
  that enqueued the job (or published the event a delivery carries). It is
  55 ASCII characters of ids and flags: no user data. Written by
  `jobs::enqueue`, `EventBus::publish` and schedules (none: a schedule has
  no caller); read by the worker when it claims the job.
- **Events published or consumed:** none.
- **Endpoints:** none in the API, so no OAuth scope and no OpenAPI change.
  `GET /metrics` is served on a separate operator listener
  (`CAMPBOOKS_TELEMETRY__METRICS_ADDR`), like the `healthcheck` subcommand it
  is an operations surface, not part of the API, so it is outside the
  operation catalogue.
- **AI:** no AI use. The AI gateway is instrumented: its spans and metrics
  carry the role, the operation, the provider's address and the model, never
  prompts, messages, embeddings input or answers.

## Decisions

### One instrumentation model: `tracing` spans + OpenTelemetry metrics API

Spans stay `tracing` spans (the whole codebase already uses them, and log
lines inherit their fields). `tracing-opentelemetry` turns them into
OpenTelemetry spans; special fields `otel.name`, `otel.kind` and
`otel.status_code` set the span name, kind and status. Metrics use the
OpenTelemetry metrics API (`Meter`, `Histogram`, `Gauge`, observable gauges),
so one SDK, one resource (`service.name`, `service.version`,
`campbooks.role`) and one set of attribute names serve both signals.
Attribute names follow the OpenTelemetry semantic conventions where one
exists (`http.route`, `http.response.status_code`, `gen_ai.*`,
`db.client.connection.*`, `server.address`, `error.type`); Campbooks-specific
ones are `campbooks.*`, `job.*`, `workspace.id`, `principal.kind`.

### Export: OTLP and/or a Prometheus page, both off by default

| Setting | Default | Effect |
| --- | --- | --- |
| `CAMPBOOKS_TELEMETRY__OTLP_ENDPOINT` | unset | base URL of an OTLP/HTTP receiver (`http://collector:4318`); traces go to `/v1/traces`, metrics to `/v1/metrics` every 60 s |
| `CAMPBOOKS_TELEMETRY__OTLP_HEADERS` | unset | secret, `key=value,key=value` (e.g. an auth header for a hosted collector) |
| `CAMPBOOKS_TELEMETRY__SAMPLE_RATIO` | `1.0` | share of new traces kept (parent-based: a caller's sampled flag wins) |
| `CAMPBOOKS_TELEMETRY__METRICS_ADDR` | unset | address of a separate listener serving `GET /metrics` (Prometheus text), e.g. `0.0.0.0:9464` |

- With neither set (the self-host default) there is no exporter, no meter
  provider (every instrument is the API's no-op) and the tracer samples
  nothing. Spans still get W3C trace ids (taken from an incoming
  `traceparent` when there is one), so `trace_id` lands in the logs and an
  outbound call carries the caller's trace on. Nothing else runs.
- `OTEL_SERVICE_NAME` and `OTEL_RESOURCE_ATTRIBUTES` are honoured (the SDK's
  environment detector), so the cloud can add `deployment.environment` or a
  replica id without a Campbooks setting.
- Export runs on the SDK's own threads (batch span processor, periodic
  metric reader) with a blocking HTTP client, off the request path. A
  collector that is down costs dropped telemetry, never a slower request.
  On shutdown the providers flush with a short timeout.
- ADR 0006 keeps cloud-only telemetry out of this repo. OTLP is not
  cloud-only: it is an open protocol any self-hoster can point at their own
  collector, so the exporter lives here, off by default, and the cloud only
  supplies the endpoint and headers. Anything that is cloud-specific (the
  error-reporting hook, vendor dashboards) stays in the private build.
- The metrics listener has no authentication and serves nothing else. The
  compose file does not publish it; `deploy/README.md` says to keep it on a
  private network.

### Trace context

- **Incoming.** The request middleware extracts `traceparent`/`tracestate`
  with the W3C propagator and makes it the request span's parent.
  Trusting a remote sampled flag means a client can ask for its own trace to
  be kept; that costs one trace, and the cloud's edge proxy can strip the
  header. Documented.
- **Outgoing.** `kernel::telemetry::inject` writes the current span's
  context into a `HeaderMap`; the AI gateway and the JWKS client call it on
  every request.
- **Jobs.** Enqueueing stores the current `traceparent` in
  `kernel.jobs.trace_context`. The job span **links** to it rather than
  becoming its child: a job may run minutes later, retry many times, or be
  one of many deliveries of one event, and a link keeps each attempt its own
  trace while still pointing back to the request.

### Spans and their attributes

| Span | Name (`otel.name`) | Attributes |
| --- | --- | --- |
| HTTP request (server) | `{method} {route}` (`{method}` when unmatched) | `http.request.method`, `http.route`, `http.response.status_code`, `request_id`, `trace_id`, `workspace.id` (when the route has one), `principal.kind` (`user`/`client`, when authenticated) |
| Job attempt (consumer) | `job {kind}` | `job.id`, `job.kind`, `job.queue`, `job.attempt`, `job.outcome`, `workspace.id`, `trace_id`; link to the enqueuing span |
| AI call (client) | `{operation} {model}` | `gen_ai.operation.name` (`chat`, `embeddings`, `decide`), `campbooks.ai.role`, `gen_ai.request.model`, `gen_ai.response.model`, `server.address`, `gen_ai.usage.input_tokens`, `gen_ai.usage.output_tokens`, `error.type` |
| IdP fetch (client) | `GET idp` | `server.address`, `http.response.status_code`, `error.type` |

`workspace.id` and `principal.kind` are recorded by the kernel's `Workspace`
and `Scoped` extractors into the current span, so every module gets them
without writing code. The request span no longer carries the URI: a query
string can hold an email address, a path segment an invitation token.
Database pool waits appear as span events (sqlx's `sqlx::pool::acquire`
event with `acquired_after_secs`, enabled at DEBUG for the trace pipeline
only).

### Metrics

| Instrument | Kind, unit | Attributes |
| --- | --- | --- |
| `http.server.request.duration` | histogram, s | `http.request.method`, `http.route`, `http.response.status_code`, `error.type` (the problem's stable `code`, e.g. `not_found`, on 4xx/5xx; never its translated text) |
| `campbooks.job.duration` | histogram, s | `job.kind`, `job.queue`, `job.outcome` (`succeeded`, `retried`, `failed`, `lost`) |
| `campbooks.job.wait` | histogram, s | `job.kind`, `job.queue` (from ready to claimed) |
| `campbooks.jobs` | gauge | `job.queue`, `job.state` (`ready`, `scheduled`, `running`, `failed`) |
| `campbooks.jobs.oldest_ready_age` | gauge, s | `job.queue` |
| `gen_ai.client.operation.duration` | histogram, s | `gen_ai.operation.name`, `campbooks.ai.role`, `server.address`, `gen_ai.request.model`, `error.type` |
| `gen_ai.client.token.usage` | histogram, {token} | the above plus `gen_ai.token.type` (`input`, `output`) |
| `db.client.connection.count` | gauge | `db.client.connection.state` (`idle`, `used`) |
| `db.client.connection.max` | gauge | |
| `db.client.connection.wait_time` | histogram, s | |
| `campbooks.build.info` | gauge (1) | `service.version`, `campbooks.role` |

- **Cardinality.** Labels are route templates, configured names (queues,
  job kinds, roles, models, provider addresses) and small enums. Ids, paths,
  emails and error messages never become labels; `error.type` is a fixed
  word (`timeout`, `connect`, `status_5xx`, `status_4xx`, `invalid_response`,
  `config`).
- **Queue depth** comes from one grouped query over the pending/running/failed
  partial indexes, run by each worker every 15 s while metrics are exported
  (not at all otherwise). Several workers report the same value; dashboards
  take the max.
- **Buckets** for durations are explicit (5 ms … 10 min) because the SDK's
  default buckets are tuned for milliseconds.

### Where the code lives

- `campbooks-kernel::telemetry`: field names, the W3C propagator helpers
  (`set_parent_from`, `inject`, `current_traceparent`, `link_to`,
  `record_trace_id`), `Metrics` (the HTTP and job instruments, held in
  `AppState`; the no-op until the server installs a meter provider), and
  `TelemetryConfig` lives in `kernel::config` with the others. The kernel
  stays domain-free; nothing here names a module.
- `campbooks-ai`: its own instruments, created from the global meter when the
  gateway is built.
- `campbooks-server::telemetry`: builds the subscriber (fmt layer, OTel layer,
  pool-wait layer, each with its own filter), the SDK providers and
  exporters, the `/metrics` router, the pool gauges, and shuts them down. The
  request middleware replaces `tower-http`'s `TraceLayer`.

## Alternatives considered

### A. Prometheus only, with the `metrics` crate

`metrics` + `metrics-exporter-prometheus` (MIT, small) behind a `/metrics`
listener, traces left to logs.
- For: the lightest option; Prometheus text is what most self-hosted tools
  read; no collector needed.
- Against: no traces, so the request → job → AI-provider path the issue asks
  for is impossible; the cloud's collector would have to scrape every
  replica of every role (service discovery per role) instead of receiving
  pushes; a second instrumentation API next to `tracing` if traces come
  later.

### B. OTLP only

The OpenTelemetry SDK with the OTLP exporter for traces and metrics, no
scrape endpoint.
- For: one export path, pushes from every replica, what the cloud wants.
  Prometheus 3 can even receive OTLP metrics directly.
- Against: a self-hoster who just wants a Grafana panel must run a collector
  (or Prometheus with its OTLP receiver turned on) and point traces
  somewhere that accepts them; the issue explicitly asks for a Prometheus
  endpoint behind config.

### C. Both, from one OpenTelemetry SDK (chosen)

The same instruments feed an OTLP periodic reader and/or a Prometheus pull
reader, chosen by two independent settings.
- For: one instrumentation API; the cloud pushes, the self-hoster scrapes;
  each is a single setting and nothing runs when neither is set.
- Against: the heaviest dependency set of the three (the OTel SDK, `prost`
  for the OTLP protobuf encoding, the `prometheus` crate for the text
  format). Kept down by HTTP/protobuf only (no `tonic`/gRPC, no second HTTP
  stack: blocking reqwest 0.13 is already in the graph) and `prometheus`
  without default features (no protobuf). `opentelemetry-prometheus` is
  maintained by the OpenTelemetry project but its README nudges new projects
  towards OTLP; if it is ever dropped, the scrape endpoint can render the
  same `ManualReader` data itself without touching any instrument.

### D. A Sentry-compatible SDK (what the Rails app on `main` does)

The Rails app reports errors and 10 %-sampled performance traces to a
self-hosted GlitchTip through `sentry-ruby`, with `send_default_pii = false`.
- For: errors and traces in one tool the team already runs.
- Against: a vendor protocol rather than an open one, no metrics, and
  GlitchTip's tracing is limited. **Not copied.** Two ideas from it are
  kept because they win on their own: privacy by default (here enforced by a
  test rather than a flag) and light sampling in the cloud (here a setting,
  `CAMPBOOKS_TELEMETRY__SAMPLE_RATIO`, rather than a constant). Error
  reporting to GlitchTip stays a separate, cloud-only hook (non-goal).

### Smaller choices

- **Async vs thread-based export.** The SDK's tokio-based processors are
  behind `experimental_*` features; the default thread-based ones with a
  blocking client are the supported path.
- **Job link vs parent.** See Trace context: a parent would make a
  retried job's trace span hours and mix unrelated deliveries.
- **Trace ids in logs.** A custom JSON formatter could print `trace_id` at
  the top level of every line; recording it as a field of the request and
  job spans gets it into every line inside them with the stock formatter.
- **Queue depth as an observable gauge with an async query** is not possible
  (callbacks are synchronous), so the worker samples on a timer and records
  a synchronous gauge.

## Risks

- **A collector that is down** fills the batch queue (2048 spans), then
  spans are dropped; requests are unaffected. Export errors are logged by
  the SDK at WARN.
- **Cost of the OTel layer when nothing is exported**: spans are built and
  immediately dropped by the `AlwaysOff` sampler (not benchmarked; expected
  to be microseconds per span), accepted for having trace ids in logs and
  on outbound calls.
- **Privacy regressions** (someone adds `email = %…` to a span): the privacy
  test covers the request and job paths that exist today; the rule is in
  `AGENTS.md` (no personal data or secrets in span fields or log fields).
