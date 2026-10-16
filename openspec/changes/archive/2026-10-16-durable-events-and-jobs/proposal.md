# Proposal

## Why

ADR 0011 makes background work durable and keeps it in Postgres: domain
events are written in the same transaction as the change they describe,
consumers are idempotent, and work is shared fairly between workspaces. Today
the kernel's `EventBus` is an in-memory broadcast channel: an event published
just before a crash is lost, a slow subscriber drops events, and there is no
way to run anything in the background, retry it or schedule it. Mail sync,
the AI pass, projections and maintenance (the next milestones) all need this
first.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/19. The
queue choice is ADR 0012 (our own queue on `FOR UPDATE SKIP LOCKED`, after
evaluating `apalis`).

## What Changes

- **Event log.** `DomainEvent` gains an explicit schema `VERSION` and the
  `workspace` it belongs to. `EventBus::publish(&mut tx, &event)` appends the
  event to `kernel.events` (retained) and, in the same transaction, enqueues
  one delivery job per registered consumer. **BREAKING** for kernel callers:
  the in-memory `publish`/`subscribe` API is removed (no module used it).
- **Consumers** are `EventHandler` types named `<module>.<handler>`,
  registered in the module's descriptor. `campbooks-server events replay
  <consumer> [--since <time|event id>]` re-delivers logged events to one
  consumer.
- **Jobs.** `Job` types (`NAME`, `QUEUE`, `MAX_ATTEMPTS`, `run`), enqueued
  with `jobs::enqueue(&mut tx, &job, options)`: workspace, delay, priority,
  `unique_key` (dedupe) and `ordering_key` (serial execution). Retries back
  off exponentially with jitter; `JobError::Fatal` and exhausted jobs are
  kept as failed; succeeded jobs are pruned after a retention window.
- **Workers** claim fairly across workspaces (a per-workspace limit and
  turn-taking claims), lease jobs with a heartbeat, reclaim expired leases,
  wake on `LISTEN/NOTIFY`, and drain gracefully on shutdown. A job's
  transaction carries `campbooks.workspace_id` and commits with its
  completion.
- **Schedules.** `Background::schedule::<J>(every)` enqueues each
  epoch-aligned tick once across all workers. The kernel schedules
  `kernel.prune_jobs` hourly.
- **Process roles.** `campbooks-server all` (default; the image's command),
  `api`, `worker`; `serve` stays as a hidden alias of `all`. New
  `CAMPBOOKS_JOBS__*` settings.
- **Kernel schema.** Tables live in Postgres schema `kernel`, migrated by
  `<ts>_kernel__<desc>.sql`. `cargo xtask arch` accepts kernel migrations,
  checks that every migration touches only its owner's schema, reserves the
  name `kernel`, and rejects misnamed or duplicate background registrations.

## Capabilities

### New Capabilities

- `events-and-jobs`: publishing domain events durably, delivering them to
  consumers, enqueueing and running background jobs (retries, uniqueness,
  ordering, fairness, leases, schedules), replay, and the process roles that
  run them.

### Modified Capabilities

None. `system` behaviour is unchanged; its module descriptor only declares
that it has no background work.

## Impact

- Modules: `campbooks-kernel` (events, new `jobs`, config, module contract,
  state, test config), `campbooks-server` (roles, registry, `events replay`),
  `campbooks-system` (empty `background`), `xtask` (migration and
  registration rules). No new module, no cross-module dependency, no
  `architecture.toml` entry.
- `kernel::Module` gains a `background` field; `AppState::with_registry`
  attaches the modules' registry.
- Data: new schema `kernel` (`events`, `jobs`, `schedules`, function
  `uuid_v7()`), owned by the kernel, not by a module.
- Dependencies: the server crate now uses the workspace's existing `chrono`.
  No new crate.
- Self-hosters: the image runs `all` (API + worker) instead of `serve`; new
  optional `CAMPBOOKS_JOBS__*` settings. Nothing to do on upgrade: the
  migration runs on startup.
- OpenAPI: unchanged (no endpoint).

## Non-goals

- Any real event or job in a domain module. None exists yet; the first come
  with `workspaces` and `connectors`.
- The `sync` role of ADR 0011 and per-provider rate limits (ADR 0010); both
  arrive with the connectors.
- Row-level security policies. Jobs set `campbooks.workspace_id`; the
  policies come with the `workspaces` module.
- Event upcasting (reading old payload versions) and per-event ordering keys
  for deliveries.
- A queue dashboard, metrics exporter or admin API. Jobs are rows to query;
  workers log a span per job and periodic counters.
- Cron expressions. Schedules are fixed intervals aligned to the epoch.
- A health endpoint for worker-only processes.
