# 0012 — Our own Postgres job queue; events fan out to jobs at publish time

**Status:** Proposed · 2026-10-12

## Context
ADR 0011 asks for a durable queue and outbox in Postgres (issue #19): domain
events written in the same transaction as the change, idempotent consumers,
work partitioned by workspace for fairness, no broker to operate. ADR 0002
left the queue choice open. Mail sync, the AI pass, projections and
maintenance all run on it, so it has to give us:

1. **Transactional enqueue** with the caller's own `sqlx` transaction: no
   job without its data change, no data change without its job.
2. **An event log** that is retained, so projections can be rebuilt by
   replaying it, and **fan-out** to every consumer of an event.
3. **Per-workspace fairness**: one workspace with a large backlog must not
   starve the others, and no workspace may hold every worker.
4. **Retries** with exponential backoff and jitter, an explicit
   "don't retry" outcome, and dead jobs kept for inspection.
5. **Leases** that survive long jobs (heartbeat) and are reclaimed when a
   worker dies; a worker that lost its lease must not commit.
6. **Uniqueness** (idempotent enqueue) and **ordering keys** (jobs about the
   same thing run one at a time, in order).
7. **Recurring schedules** that run once per tick however many workers run.
8. **The workspace on the connection** (`SET LOCAL campbooks.workspace_id`)
   for the row-level security of ADR 0011.
9. One process by default, separate worker processes when scaling.

## Options

**`apalis` with `apalis-postgres`.** A mature job framework (tower
middleware, retries, cron via `apalis-cron`, a web UI). Measured against the
list:

- Version: 1.0.0 was published on the day of this decision; the newest
  release at least two weeks old (our supply-chain rule, ADR 0007) is
  `apalis-postgres` 1.0.0-rc.9, a release candidate.
- (1) Possible but not first-class: `apalis_postgres::sink::push_tasks` takes
  any executor, so a job can be pushed inside our transaction, but the
  documented path pushes through the storage's own pool.
- (2) Not provided. It is a job queue, not an event log; fan-out and replay
  would be ours on top.
- (3) Not provided. Jobs are claimed per queue by priority and time; there is
  no notion of a tenant to be fair across, and adding one means replacing its
  fetch query (a SQL function in its own migrations).
- (6) Idempotency keys exist, but the unique index covers finished jobs too
  (a key can be used once, ever). There are no ordering keys.
- (8) Not provided; handlers get no transaction.
- It owns schema `apalis` through 23 migrations of its own, PL/pgSQL
  functions included, and stores payloads as bytes. Our migrations,
  `cargo xtask arch` checks and `.sqlx` offline data would not cover it.

Using it means fighting it on (1), (2), (3), (6) and (8), which are the
reasons we want a queue at all.

**A small queue of our own on `FOR UPDATE SKIP LOCKED`.** About 2,000 lines
(most of them doc comments) plus 1,000 of tests, in `campbooks-kernel`, against tables in a `kernel` schema
that our own migrations, arch rules and compile-time checked queries cover.
Postgres features do the hard parts: `SKIP LOCKED` for concurrent claims,
partial unique indexes for uniqueness, advisory transaction locks for
ordering, `LISTEN/NOTIFY` for low latency.

## Decision
Build our own queue in the kernel.

- **Schema `kernel`** holds infrastructure, not a module:
  `kernel.events` (the log), `kernel.jobs` (the queue), `kernel.schedules`
  (last tick per schedule). Migrations are `<ts>_kernel__<desc>.sql` and touch
  only `kernel`; module migrations may not touch it (`cargo xtask arch`).
- **Events fan out at publish time.** `EventBus::publish(&mut tx, &event)`
  inserts the event row and one job per registered consumer in the caller's
  transaction. Consumers are known at startup: modules register them in
  their `Module` descriptor (`Background::on_event::<H>()`), named
  `<module>.<handler>`. We rejected offset-based consumers (each consumer
  tracks the last event id it read) because ids are assigned at insert, not
  commit: a reader can see event 11 commit before event 10 and skip 10 for
  good. Fan-out has no such gap. **Replay** re-enqueues deliveries to one
  consumer from the log (`campbooks-server events replay <consumer> --since`).
- **Event ids are UUIDv7**, like every id in the system: generated without
  coordination, time-ordered for `--since <event id>`, and never relied on
  for ordering delivery (fan-out removes the need). A `bigserial` would only
  add a dense order we deliberately do not depend on. `kernel.uuid_v7()`
  generates them in SQL until Postgres 18's `uuidv7()` is our minimum.
- **One registration style.** Jobs (`trait Job`, `run(self, ctx)`) and event
  handlers (`trait EventHandler`, `handle(ctx, event)`) are types with a
  stable `NAME`, a `QUEUE` and `MAX_ATTEMPTS`, registered on the module's
  `Background` (`.job::<J>()`, `.on_event::<H>()`, `.schedule::<J>(every)`).
  Startup (and `cargo xtask arch`) rejects names not prefixed by their module,
  duplicates and bad queue names.
- **A job's database work commits with its completion.** The handler's
  `ctx.tx()` opens a transaction with `campbooks.workspace_id` set; the
  worker marks the job succeeded inside it, guarded by the lease (worker id
  and attempt). A failed handler, or one whose lease was taken over, rolls
  back. Delivery is at least once; database-only handlers take effect once.
- **Fair claiming.** One claim statement walks the workspaces with ready work
  (a recursive index skip-scan), skips workspaces already running
  `CAMPBOOKS_JOBS__PER_WORKSPACE` jobs in that queue, takes each workspace's
  next jobs by priority and age, and interleaves them in turns: every
  workspace's first job before anyone's second, the workspace served least
  recently first. A workspace with 1,000 ready jobs delays another's single
  job by at most one claim.
- **Retries:** `JobError::Retry` (optionally with a `Retry-After`) backs off
  exponentially from `CAMPBOOKS_JOBS__RETRY_BASE_MS` to
  `CAMPBOOKS_JOBS__RETRY_MAX_SECONDS` with 50–100 % jitter;
  `JobError::Fatal`, a panic on the last attempt, an undecodable payload or
  an unknown kind mark the job `failed` (dead). Dead jobs stay; succeeded ones
  are pruned after `CAMPBOOKS_JOBS__RETENTION_HOURS` by the hourly
  `kernel.prune_jobs` schedule. The event log is never pruned.
- **Leases:** a claim leases the job for `CAMPBOOKS_JOBS__LEASE_SECONDS`,
  renewed every third of that while it runs; workers reclaim expired leases.
- **Schedules** tick on epoch-aligned intervals. `kernel.schedules` records
  the last tick each schedule enqueued (`INSERT … ON CONFLICT … WHERE
  last_tick < new`), so any number of workers enqueue a tick once.
- **Roles:** `campbooks-server all` (default, the image's command), `api`,
  `worker`. Enqueues `NOTIFY kernel_jobs`; idle workers `LISTEN` and poll
  every `CAMPBOOKS_JOBS__POLL_INTERVAL_MS` as a safety net. On shutdown a
  worker stops claiming, waits `CAMPBOOKS_JOBS__SHUTDOWN_GRACE_SECONDS`, then
  hands unfinished jobs back without counting the attempt.

## Consequences
- Self-hosting is still one container plus Postgres. Workers scale out by
  running more `worker` processes against the same database.
- We own about 2,000 lines of queue code and its tests. The interface
  (`publish`, `enqueue`, `Job`, `EventHandler`) hides the storage, so moving
  to a broker later (ADR 0011) changes the kernel, not the modules.
- The claim statement's cost grows with the number of workspaces with ready
  work in a queue, not with the backlog; very large backlogs of future-dated
  jobs in one queue slow the skip-scan and are the signal to partition.
- Per-workspace limits are checked when claiming, so workers claiming at the
  same instant can exceed them by a job each.
- Deliveries of one event type are not ordered relative to each other; a
  consumer that needs order uses an ordering key on follow-up jobs (an
  ordering key per event is a follow-up).
- No queue UI: jobs are rows to query, and workers log a span per job (kind,
  queue, attempt, workspace) plus periodic counters.

## Alternatives considered
- `apalis` + `apalis-postgres` — see Options.
- Offset-based event consumers over the log — the commit-order gap above.
- A broker (Kafka, NATS, RabbitMQ) — rejected by ADR 0011 for self-hosting.
