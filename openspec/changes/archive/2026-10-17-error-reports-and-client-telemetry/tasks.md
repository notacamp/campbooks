# Tasks

## 1. Kernel

- [x] 1.1 `TelemetryConfig`: `error_dsn`, `environment`, `client_collector_url`, `client_collector_key`, blank means unset; startup checks for the collector URL and environment
- [x] 1.2 `redact::error_kind` and `redact::looks_like_an_id`, with tests

## 2. Server error reports

- [x] 2.1 `error_reports`: client options (no PII, no integrations, release, environment), `ErrorReportLayer`, `scrub`, `campbooks.errors.reported`
- [x] 2.2 `Telemetry` starts it, installs the layer, flushes on shutdown
- [x] 2.3 `campbooks-server telemetry test-error`
- [x] 2.4 Privacy tests: a loaded event through `scrub`; a request, a job and a panic through the layer and client into a test transport; no address, subject, body, attachment name, token, cookie or IP in any report, ids present

## 3. The app's telemetry

- [x] 3.1 `system.report_client_telemetry` (`client_telemetry` slice, `system.client_report` resource): 404 without a collector, scrubbing, forwarding with the key, item counter; tests for the scrubbed payload, the caps, the view rule, 401, 404 and the forward
- [x] 3.2 Client `lib/telemetry` (errors, router catches, web vitals, batching, stop on 404) over `backgroundApi`; tests

## 4. Connectors

- [x] 4.1 `campbooks.connector.syncs` and `campbooks.connector.overdue_streams`

## 5. Docs and gates

- [x] 5.1 `deploy/README.md` (Monitoring), `deploy/.env.example`, CHANGELOG
- [x] 5.2 OpenAPI regenerated, client types regenerated, `.sqlx` regenerated, gates green
