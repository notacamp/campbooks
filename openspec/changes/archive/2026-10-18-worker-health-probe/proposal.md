## Why

`campbooks-server healthcheck` probes the API's health endpoint, which a
`worker` process does not serve. A separate worker container therefore ran
with its health check disabled, and a stuck job loop or lost database went
unseen (campbooks-cloud#185, from the durable-events-and-jobs change's task
5.4).

## What Changes

- **Liveness.** The worker's loop runs `SELECT 1` every 10 seconds and, when
  it succeeds, beats (`jobs::Liveness`). A worker counts as alive for a minute
  after its last beat (`ALIVE_FOR`): several beats, and longer than a poll
  interval or a statement's time limit.
- **The endpoint.** A worker-only process serves `GET /healthz` on
  `CAMPBOOKS_JOBS__HEALTH_ADDR` (default `127.0.0.1:8081`, inside the
  container): 200 while alive, 503 otherwise. `all` and `api` keep
  `/api/v1/system/health`.
- **The probe.** `campbooks-server healthcheck --worker` probes it (its port
  from `CAMPBOOKS_JOBS__HEALTH_ADDR`, else 8081). The runtime image has no
  shell or curl, so the probe is the binary itself, as for the API.
- **Self-hosting:** the guide gives a worker service
  `healthcheck: { test: ["CMD", "campbooks-server", "healthcheck", "--worker"] }`.
  The hosted compose (campbooks-cloud) should do the same.

## Impact

- Spec: events-and-jobs (Process roles).
- Code: kernel `jobs::worker` (`Liveness`), `config` (`JobsConfig::health_addr`),
  server `main` (the endpoint, `healthcheck --worker`).
- Docs: `deploy/README.md`.
