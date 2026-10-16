# Design

## Context

`kernel::events` is an in-memory broadcast bus (`EventBus::publish` /
`subscribe`) that no module uses yet. `AppState` carries it; `Module` is
`{ name, scopes, router }`; `campbooks-server` has one long-running command,
`serve`. ADR 0011 (one binary in roles, events from an outbox, fairness per
workspace, `SET LOCAL` of the workspace) and ADR 0012 (our own queue,
fan-out at publish time) set the shape. See proposal.md for why now.

## Data ownership and events

- **Schema `kernel`**, owned by `campbooks-kernel`, not a module. Written
  only through the kernel's API. Migration
  `20261012100000_kernel__events_and_jobs.sql`.
- **No module schema changes.** `system` gains no table.
- **Events published or consumed:** none by a module in this change. The
  kernel defines the mechanism and tests it with test-only events.
- **Endpoints:** none, so no OAuth scope. The replay command is an operator
  CLI against the database, not an API.
- **AI:** none.

### Tables

`kernel.events` — the log, append-only, never pruned.

| Column | Type | Notes |
| --- | --- | --- |
| `id` | `uuid` PK | UUIDv7 (`kernel.uuid_v7()`) |
| `name` | `text` | `DomainEvent::NAME` |
| `version` | `integer > 0` | `DomainEvent::VERSION` |
| `workspace_id` | `text` null | `DomainEvent::workspace()`; null = instance-wide |
| `payload` | `jsonb` | the event, serialized |
| `occurred_at` | `timestamptz` | transaction time of the publish |

Index: `(name, occurred_at)` for replay.

`kernel.jobs` — the queue.

| Column | Notes |
| --- | --- |
| `id uuid` PK | UUIDv7 |
| `seq bigint identity` | enqueue order; orders an `ordering_key` |
| `queue`, `kind text` | `Job::QUEUE`/`NAME`, or the consumer's |
| `payload jsonb` | the job (`{}` for deliveries) |
| `workspace_id text`, `workspace_key` | `workspace_key` = `coalesce(workspace_id, '')` (generated): the fairness bucket |
| `event_id uuid` | the delivered event, for deliveries |
| `unique_key`, `ordering_key text` | see below |
| `priority smallint` | higher first within a workspace |
| `state text` | `pending`, `running`, `succeeded`, `failed` |
| `run_at`, `attempts`, `max_attempts` | scheduling and retries |
| `locked_by`, `locked_until` | the lease; set iff `running` (CHECK) |
| `last_error text` | truncated to 2,000 characters |
| `created_at`, `updated_at`, `started_at`, `finished_at` | |

Indexes (all partial where it pays):

- `jobs_ready_idx (queue, workspace_key, priority DESC, run_at, seq) WHERE state = 'pending'` — claiming.
- `jobs_running_idx (queue, workspace_key) WHERE state = 'running'` — per-workspace limit.
- `jobs_last_started_idx (queue, workspace_key, started_at DESC) WHERE started_at IS NOT NULL` — who was served least recently.
- `jobs_lease_idx (locked_until) WHERE state = 'running'` — reclaiming.
- `jobs_unique_key_idx UNIQUE (kind, unique_key) WHERE unique_key IS NOT NULL AND state IN ('pending','running')` — dedupe.
- `jobs_ordering_idx (ordering_key, seq) WHERE ordering_key IS NOT NULL AND state IN ('pending','running')` — serial keys.
- `jobs_finished_idx (state, finished_at) WHERE state IN ('succeeded','failed')` — pruning, inspection.

`kernel.schedules (name PK, last_tick bigint, updated_at)` — the last tick
each schedule enqueued.

## Decisions

### API (one registration style)

```rust
// Events: declared by the publishing module.
impl DomainEvent for MessageReceived {
    const NAME: &'static str = "mail.message_received";
    const VERSION: u16 = 1;                       // no default, on purpose
    fn workspace(&self) -> Option<&str> { Some(&self.workspace) }
}
state.events.publish(&mut tx, &event).await?;    // in the change's transaction

// A consumer, in any module.
impl EventHandler for TrackSender {
    type Event = MessageReceived;
    const NAME: &'static str = "parties.track_sender";
    const QUEUE: &'static str = "events";
    async fn handle(ctx: &mut JobContext, event: Event<MessageReceived>) -> Result<(), JobError> { … }
}

// A job.
impl Job for SyncMailbox {
    const NAME: &'static str = "mail.sync_mailbox";
    const QUEUE: &'static str = "sync";
    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> { … }
}
jobs::enqueue(&mut tx, &SyncMailbox { .. },
    EnqueueOptions::for_workspace(ws).unique(mailbox_id).ordered_by(mailbox_id)).await?;

// Registration, in the module descriptor.
Module { name: "mail", …, background: Background::new()
    .job::<SyncMailbox>()
    .on_event::<TrackSender>()
    .schedule::<RefreshTokens>(Duration::from_secs(900)) }
```

Both `publish` and `enqueue` take a `Transaction`, not a bare connection, so
"in the caller's transaction" is checked by the compiler. The brief's sketch
(`jobs.enqueue(...)`) became a free function: the enqueuer has no state.

`Registry::build(&modules())` collects every module's `Background` plus the
kernel's own (`kernel.prune_jobs`, hourly). It rejects names not of the form
`<owner>.<snake>` of the declaring module, duplicates, queue names that are
not snake case, `MAX_ATTEMPTS = 0`, an event `VERSION` of 0 and schedules
under one second. The server fails to start on a bad registry and
`cargo xtask arch` reports the same errors.

### Publish = fan-out

One statement inserts the event and, from `unnest` of the registered
consumers, one job per consumer with `event_id`, the event's workspace and
`unique_key = event id`. The unique key makes a replay of an event whose
delivery is still pending a no-op. Each distinct queue is `NOTIFY`d. An event
with no consumer is still logged (replayable once a consumer exists).

A delivery job loads its event row, checks name and version (a mismatch is
fatal: upcasting is a non-goal), decodes the payload and calls the handler
with `Event { id, version, workspace, occurred_at, payload }`.

### Replay

`EventBus::replay(db, consumer, from)` inserts one delivery per logged event
of the consumer's event name from `from` (beginning, an instant, or an event
id inclusive) with `ON CONFLICT … DO NOTHING` on the unique index, in one
transaction, and notifies. It returns how many it enqueued.
`campbooks-server events replay <consumer> [--since …]` wraps it.

### Running a job

1. **Claim** (one statement): a recursive skip-scan lists the workspaces with
   ready work in the queue; workspaces with `per_workspace` running jobs drop
   out; for each remaining one a `LATERAL … FOR UPDATE SKIP LOCKED` takes its
   next jobs (priority, run_at, seq), skipping jobs whose `ordering_key` has
   an earlier unfinished job; `row_number()` per workspace gives each job its
   turn; the claim takes `limit` jobs by turn, priority, the workspace's last
   start (nulls first), age. Claimed jobs become `running` with the lease and
   `attempts + 1`.
2. **Execute** in a tracing span (`job_id`, `kind`, `queue`, `attempt`,
   `workspace`), renewing the lease every third of it. `ctx.tx()` opens the
   transaction lazily and runs `set_config('campbooks.workspace_id', ws,
   true)` when the job has a workspace.
3. **Complete**: on `Ok`, mark the job `succeeded` in the job's transaction
   (or directly when none was opened) **only where `locked_by` and
   `attempts` still match**, then commit. Zero rows means the lease was taken
   over: roll back, count it as lost.
4. **Fail**: on `Err`, roll back, then set `pending` with
   `run_at = now() + after` (`Retry-After`) or
   `min(max, base · 2^(attempt-1)) · (0.5 + random()/2)`, or `failed` when
   fatal or out of attempts. Same lease guard. A panicking handler counts as
   a retryable failure; an unknown kind or undecodable payload is fatal.

**Leases**: every upkeep pass (each poll interval, clamped to 1–5 s) moves
`running` jobs whose `locked_until` passed back to `pending` (or `failed` on
their last attempt), with `last_error = 'lease expired: …'`.

**Ordering keys**: `enqueue` with an ordering key first takes
`pg_advisory_xact_lock(hash(key))`, so producers of one key are serialized
until commit and `seq` order equals commit order for that key. A failed
(dead) job releases its key, so one poison job cannot block the key forever.

**Uniqueness**: `INSERT … ON CONFLICT (kind, unique_key) WHERE … DO NOTHING`
returns `Enqueued::Duplicate` while a job with the key is pending or running;
the key is free again once it finishes.

### Schedules

Each upkeep pass, for each schedule: `INSERT INTO kernel.schedules … ON
CONFLICT (name) DO UPDATE … WHERE last_tick < floor(epoch(now()) / every)`.
Only the worker whose statement advanced the tick enqueues, in the same
transaction, with `unique_key = 'schedule:<name>'`. Deviation from the brief
("a unique key per tick"): the tick guard lives in `kernel.schedules`, which
also covers the case where the previous tick's job already finished, and the
per-schedule unique key coalesces ticks while a run is still pending or
running, so a slow schedule never piles up.

### Worker loop and roles

`Worker::run(shutdown)` keeps `concurrency` jobs in a `JoinSet`. It wakes on
a `LISTEN kernel_jobs` notification (a background task turns them into a
`Notify`, reconnecting after errors), a finished job, the poll interval, or
upkeep. Database errors are logged and retried, never fatal. On shutdown it
stops claiming, waits `shutdown_grace_seconds` for running jobs, aborts the
rest and releases them (`pending`, `attempts - 1`, so the interruption does
not count). Counters (claimed, succeeded, retried, failed, released, lost)
are logged every 5 minutes and on stop.

`campbooks-server`:

| Command | API | Worker |
| --- | --- | --- |
| `all` (default; `serve` is a hidden alias) | yes | yes |
| `api` | yes | no |
| `worker` | no | yes |

All roles run migrations when `CAMPBOOKS_RUN_MIGRATIONS` is true (sqlx's
migration lock makes concurrent starts safe). Ctrl-C/SIGTERM flips a shared
watch channel; if either part ends on its own, the other is stopped too.

### Configuration (`CAMPBOOKS_JOBS__*`)

| Setting | Default | Check |
| --- | --- | --- |
| `CONCURRENCY` | 4 | ≥ 1 |
| `PER_WORKSPACE` | 2 | ≥ 1 |
| `POLL_INTERVAL_MS` | 5000 | ≥ 10 |
| `LEASE_SECONDS` | 60 | ≥ 3 |
| `RETENTION_HOURS` | 168 | ≥ 1 |
| `SHUTDOWN_GRACE_SECONDS` | 8 | (below Docker's 10 s stop timeout) |
| `RETRY_BASE_MS` | 1000 | ≥ 1 |
| `RETRY_MAX_SECONDS` | 3600 | ≥ base |

The `worker` and `all` roles refuse to start on a failed check, naming the
setting.

### Architecture rules

`cargo xtask arch`:

- accepts `<ts>_kernel__<desc>.sql` and reserves `kernel` as a module name;
- per migration, strips comments and string literals and fails on
  `CREATE/ALTER/DROP SCHEMA` of another schema, on a name qualified by
  another module's schema or `kernel`, and on `search_path`;
- reports `Registry::build` errors as violations.

## Risks / Trade-offs

- [Claim cost grows with the number of workspaces with ready work] → the
  skip-scan visits each ready workspace once per claim; fine for the
  expected thousands; partitioning is the ADR 0011 answer beyond that.
- [Per-workspace limit is approximate] → concurrent claims each see the same
  running count and can exceed it by one job each. Accepted: fairness, not a
  hard quota.
- [Holding the job transaction across slow calls] → `ctx.tx()` is lazy and
  documented: call providers first, write last, or split the job.
- [`NOTIFY` lost while a listener reconnects] → the poll interval catches
  it.
- [Schema check is lexical] → a table alias spelled like another module's
  schema is a false positive; renaming the alias fixes it.
- [UUIDv7 in SQL] → `kernel.uuid_v7()` is a small SQL function; when Postgres
  18 is the minimum it becomes `uuidv7()`.

## Migration Plan

The migration creates schema `kernel`; it runs on startup as usual. The image
command changes from `serve` to `all`; deployments that pass `serve`
explicitly keep working (alias). Rollback: redeploy the previous image with
`CAMPBOOKS_RUN_MIGRATIONS=false` (its migrator refuses a database holding a
migration it does not know); the `kernel` schema is unused by it.
