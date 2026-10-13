# Tasks

## 1. Kernel

- [x] 1.1 `TelemetryConfig` (`CAMPBOOKS_TELEMETRY__*`) with startup checks naming the bad setting; verify defaults, parsing, header redaction
- [x] 1.2 `kernel::telemetry`: field names, W3C helpers (`set_parent_from`, `inject`, `current_traceparent`, `link_to`, `record_trace_id`), `Metrics` with the HTTP and job instruments in `AppState`; verify the propagator round-trip
- [x] 1.3 Migration `20261013220000_kernel__job_trace_context.sql`; enqueue, event publish and claim carry `trace_context`
- [x] 1.4 Worker: `job` span with `otel.*`, outcome and link; duration and wait histograms; queue-depth sampling while metrics are on
- [x] 1.5 `Scoped` / `Workspace` record `principal.kind` / `workspace.id` into the current span; IdP fetch span with `traceparent`

## 2. AI gateway

- [x] 2.1 Span, duration and token-usage metrics around chat, embed and decide; `traceparent` on every provider request; verify with wiremock that the header is sent and no prompt reaches a span

## 3. Server

- [x] 3.1 Request middleware replacing `TraceLayer`: span, trace context, duration metric, one log line per response
- [x] 3.2 Subscriber (fmt + OTel + pool-wait layers, separate filters), tracer and meter providers, OTLP exporters, Prometheus listener, pool gauges, build info, shutdown flush
- [x] 3.3 Privacy test: request with token, cookie, email in query and claim, plus the job it causes, through the production pipeline; no secret or address in spans or log lines; route template, principal kind, workspace id, trace id and job link present
- [x] 3.4 Metrics test: Prometheus text has route-template series and no ids

## 4. Docs and gates

- [x] 4.1 `deploy/README.md` (Monitoring), `deploy/.env.example`, `AGENTS.md` rule, CHANGELOG
- [x] 4.2 `.sqlx` regenerated, OpenAPI unchanged, gates green

## 5. Follow-ups (not in this change)

- [ ] 5.1 Error reporting hook to a Sentry-compatible service (cloud-only)
- [ ] 5.2 Dashboards and alert rules in the cloud repo
- [ ] 5.3 Spans for connector sync once the `sync` role exists
