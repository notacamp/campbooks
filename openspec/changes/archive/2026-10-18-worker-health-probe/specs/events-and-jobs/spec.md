## MODIFIED Requirements

### Requirement: Process roles
`campbooks-server` SHALL run as `all` (the HTTP API and the job worker in one
process; the default when no command is given, and the image's command),
`api` (only the HTTP API) or `worker` (only the job worker); `serve` SHALL
remain an alias of `all`. Idle workers SHALL be woken by enqueue
notifications and SHALL otherwise poll every
`CAMPBOOKS_JOBS__POLL_INTERVAL_MS`. A role that runs the worker SHALL refuse
to start, naming the setting, when a `CAMPBOOKS_JOBS__*` value is out of
range. A `worker` process SHALL answer `GET /healthz` on
`CAMPBOOKS_JOBS__HEALTH_ADDR` (default `127.0.0.1:8081`): 200 while its job
loop has reached the database (`SELECT 1`, every 10 seconds) within the last
minute, 503 otherwise. `campbooks-server healthcheck --worker` SHALL exit 0
only on a 200, for container health checks.

#### Scenario: Low latency without hot polling
- **WHEN** a worker with a 10-minute poll interval is idle and a job is enqueued
- **THEN** the job runs within two seconds

#### Scenario: Invalid concurrency
- **WHEN** the worker starts with `CAMPBOOKS_JOBS__CONCURRENCY=0`
- **THEN** it exits with an error naming `CAMPBOOKS_JOBS__CONCURRENCY`

#### Scenario: A worker's health probe
- **WHEN** a `worker` process has started and its loop reached the database, and later stops reaching it for over a minute
- **THEN** `GET /healthz` answers 200 and `healthcheck --worker` exits 0, then 503 and 1
