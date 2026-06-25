# Observability

Campbooks exposes Prometheus metrics so you can watch application health —
request throughput, error rates, latency, background-job success, and key domain
actions — in Grafana or any Prometheus-compatible stack.

Metrics are collected with [yabeda](https://github.com/yabeda-rb/yabeda) and
rendered in the Prometheus text format at **`GET /metrics`**.

## What's exposed

### HTTP (RED) — from `yabeda-rails`

| Metric | Type | Notes |
| --- | --- | --- |
| `rails_requests_total{controller,action,status,format,method}` | counter | Request rate + error rate (by `status`) |
| `rails_request_duration_seconds{…}` | histogram | Latency — p50/p95/p99 via `histogram_quantile` |
| `rails_view_runtime_seconds{…}` | histogram | View rendering time |
| `rails_db_runtime_seconds{…}` | histogram | Active Record time |

> These install only when the app runs under a real web server (Puma); they are
> intentionally absent under tests and rake tasks.

### Application — from `config/initializers/yabeda.rb`

| Metric | Type | Tags | Meaning |
| --- | --- | --- | --- |
| `campbooks_domain_events_total` | counter | `event`, `group` | Domain actions published via the Events bus (email sent, document processed, …) |
| `campbooks_job_executions_total` | counter | `job`, `status` | Background-job runs by class and `success`/`failure` |
| `campbooks_job_duration_seconds` | histogram | `job`, `status` | Background-job run time |

The `event` tag is bounded to the events registered in
`app/services/events/registry.rb`; any other name buckets as `custom`.

## Scraping

`/metrics` is **unauthenticated and must not be exposed publicly** — it is meant
to be scraped over a private network. Block it at your reverse proxy (return 403
for `/metrics` on the public host) and point Prometheus at the app's internal
address:

```yaml
scrape_configs:
  - job_name: campbooks
    metrics_path: /metrics
    static_configs:
      - targets: ["campbooks-web:3000"]   # internal service address, not the public host
```

## Cardinality

Every metric tag is bounded to a small, fixed set (event names, job classes,
`success`/`failure`, HTTP controller/action). **Do not** add per-tenant tags
(workspace, user, email): Prometheus stores one time series per unique label
combination, so an unbounded tag will exhaust it. Use logs or traces for
per-tenant debugging.

## Errors

Error and performance monitoring is handled separately by Sentry/GlitchTip
(set `SENTRY_DSN`); see `config/initializers/sentry.rb`.
