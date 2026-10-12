# Tasks

## 1. Schema and architecture rules

- [x] 1.1 Migration `20261012100000_kernel__events_and_jobs.sql`: schema `kernel`, `uuid_v7()`, `events`, `jobs` (with the claim, lease, unique, ordering and finished indexes), `schedules`; verify `uuid_v7()` yields version-7, variant-10 UUIDs
- [x] 1.2 `cargo xtask arch`: accept `kernel` migrations, reserve the name, check the schemas each migration touches, report registry errors; verify with xtask unit tests and a deliberately bad migration

## 2. Jobs

- [x] 2.1 `Job`, `JobError`, `EnqueueOptions`, `enqueue` (unique key, ordering-key advisory lock, NOTIFY); verify dedupe and ordering with DB tests
- [x] 2.2 `Background` + `Registry::build` with naming and duplicate checks and the kernel's `kernel.prune_jobs` schedule; verify with unit tests
- [x] 2.3 `JobContext` with the lazy, workspace-scoped transaction; verify the `campbooks.workspace_id` setting inside a handler
- [x] 2.4 `Worker`: fair claim, execute with heartbeat, fenced completion, retry with backoff and jitter, fatal/exhausted → failed, panics, unknown kinds, lease reclaim, schedules, NOTIFY wake-up, graceful drain; verify each with DB tests (incl. 1,000-job fairness, two workers never double-claiming, two workers never double-scheduling, a lost lease rolling back)
- [x] 2.5 `PruneJobs`: deletes old succeeded jobs only; verify with a DB test

## 3. Events

- [x] 3.1 `DomainEvent::VERSION` + `workspace()`, `Event<E>`, `EventHandler`, durable `EventBus::publish` with fan-out; verify rollback leaves nothing, commit logs the event and one delivery per consumer
- [x] 3.2 Delivery: load, check name/version, decode, handle; verify fan-out end to end and a version mismatch failing
- [x] 3.3 `EventBus::replay` and `campbooks-server events replay`; verify per-consumer re-delivery, dedupe of pending deliveries, `--since` by event id and time, unknown consumers

## 4. Server

- [x] 4.1 `JobsConfig` (`CAMPBOOKS_JOBS__*`) with startup checks; verify defaults, parsing and messages
- [x] 4.2 Roles `all` (default), `api`, `worker`, `serve` alias; shared shutdown; image `CMD ["all"]`; verify each role by running the binary against Postgres and stopping it with SIGTERM
- [x] 4.3 `campbooks_server::registry()` composed from `modules()`; `system` declares an empty `background`

## 5. Follow-ups (not in this change)

- [ ] 5.1 First real events and jobs: `workspaces` (workspace created) and `connectors` (sync jobs), with row-level security policies reading `campbooks.workspace_id`
- [ ] 5.2 `sync` role and per-provider rate limits (ADR 0010, 0011)
- [ ] 5.3 Ordering key per event delivery, and payload upcasting for old event versions
- [ ] 5.4 Operator tooling: list/retry/discard failed jobs; a health probe for worker-only processes; queue-depth metrics for autoscaling

## 6. Wrap-up

- [x] 6.1 ADR 0012 written and indexed; `docs/architecture.md`, `AGENTS.md`, `deploy/README.md`, `deploy/.env.example` updated
- [x] 6.2 OpenAPI unchanged (`cargo xtask openapi --check`); client types unchanged (no endpoint)
- [x] 6.3 CHANGELOG `[Unreleased]` updated
- [x] 6.4 Gates green: fmt, clippy, tests, `cargo xtask arch`, `cargo xtask openapi --check`, sqlx prepare check, `cargo deny`, `cargo machete`, `SQLX_OFFLINE=true cargo build`, `openspec validate --all --strict`
- [x] 6.5 UI: none (no client change)
