# Design

## Context

Before this change (`20261012100000_kernel__events_and_jobs`), the queue
deduped and ordered by key across every workspace:

- `jobs_unique_key_idx UNIQUE (kind, unique_key) WHERE unique_key IS NOT NULL
  AND state IN ('pending', 'running')`;
- `jobs_ordering_idx (ordering_key, seq) WHERE ordering_key IS NOT NULL AND
  state IN ('pending', 'running')`, serving the claim's check that no earlier
  unfinished job has the key, whoever's it is;
- the enqueue's advisory lock on `'kernel.jobs.ordering_key:' || key`.

A workspace's transaction may insert rows of its own with any key but
`schedule:%` (#207's `workspace_isolation` `WITH CHECK`). So a workspace B
that knows workspace A's key (they embed ids, which travel in URLs, events
and logs) can hold it with a row due never: A's enqueue finds a duplicate and
is dropped, and A's jobs ordered by that key wait for ever.

Data ownership is unchanged: the kernel owns schema `kernel`. No event is
published or consumed, no endpoint or scope is added, no AI is used.

## Inventory

### Indexes and constraints

| Table | Index | Kind | Before | After |
|---|---|---|---|---|
| `kernel.jobs` | `jobs_pkey` | unique | `(id)` | unchanged |
| `kernel.jobs` | `jobs_unique_key_idx` | unique, partial | `(kind, unique_key)` unfinished | dropped |
| `kernel.jobs` | `jobs_unique_key_per_workspace_idx` | unique, partial | | `(kind, unique_key, workspace_id) NULLS NOT DISTINCT` unfinished |
| `kernel.jobs` | `jobs_ordering_idx` | partial | `(ordering_key, seq)` unfinished | dropped |
| `kernel.jobs` | `jobs_ordering_per_workspace_idx` | partial | | `(ordering_key, workspace_key, seq)` unfinished |
| `kernel.jobs` | ready, running, last-started, lease, finished, event-unfinished | non-unique | | unchanged |
| `kernel.events` | `events_pkey`, `events_name_occurred_at_idx` | | no key-based dedup | unchanged |
| `kernel.schedules` | `PRIMARY KEY (name)` | unique | the tick's `ON CONFLICT (name)` | unchanged (instance-wide by nature) |

### Statements that rely on a key

| Where | What | Change |
|---|---|---|
| `jobs::insert_row` (`enqueue`, `enqueue_to`, `enqueue_in`, `enqueue_to_in`, `JobContext::enqueue*`, the schedule tick) | `ON CONFLICT (kind, unique_key) WHERE …` | `ON CONFLICT (kind, unique_key, workspace_id) WHERE …` |
| `jobs::insert_row` | `pg_advisory_xact_lock(hashtextextended('kernel.jobs.ordering_key:' \|\| key, 0))` | lock on `jsonb_build_array(workspace, key)` |
| `EventBus::replay` | `INSERT … SELECT … ON CONFLICT (kind, unique_key) WHERE …` | new target |
| `EventBus::publish` | deliveries keyed `event_id::text`, no `ON CONFLICT` (a fresh event) | unchanged |
| `Worker::claim_in` | `NOT EXISTS (… o.ordering_key = j.ordering_key AND o.seq < j.seq)` | `AND o.workspace_key = j.workspace_key` |
| `jobs::admin::retry` | `NOT EXISTS (… o.kind = j.kind AND o.unique_key = j.unique_key)` | `AND o.workspace_id IS NOT DISTINCT FROM j.workspace_id` |
| `jobs::admin::retry_kind` | same check; `row_number() OVER (PARTITION BY unique_key)` | same check; `PARTITION BY workspace_id, unique_key` |
| `Worker::heal`, `jobs::admin::discard_schedule` | `kind = $1 AND unique_key = 'schedule:' \|\| $1`, any workspace | unchanged: they still retire a workspace's forged schedule row (now harmless to the tick, but still never the schedule's) |
| `changes::request_sequencing` | deliberately no unique key | unchanged |

### Callers passing a key, and the scope each now runs in

| # | Caller | Kind | Unique key | Ordering key | Scope |
|---|---|---|---|---|---|
| 1 | schedule tick (`Worker::tick`) | each schedule's job | `schedule:<name>` | | instance-wide |
| 2 | `EventBus::publish` / `replay` | each consumer | `<event id>` | | the event's workspace, or instance-wide (account events, e.g. identity's) |
| 3 | `connectors::domain::schedule` (`run_now` from `sync_now`, the dispatcher's `enqueue_to_in`) | `connectors.sync_stream` | `connectors.sync_stream:<stream>` | same | the stream's workspace |
| 4 | `connectors::sync_stream` (a continuation) | `connectors.sync_stream` | | `connectors.sync_stream:<stream>` | the stream's workspace |
| 5 | `connectors::queue_send` (the send) | `connectors.send_message` | `send:<send>` | `send:<send>` | the send's workspace |
| 6 | `connectors::queue_send` (the give-up check) | `connectors.send_message` | `give_up:<send>` | `send:<send>` | the send's workspace |
| 7 | `assistant::purge` | `assistant.purge_workspace` | `assistant.purge:<workspace>` | | that workspace |
| 8 | `attention::purge_workspace` | `attention.purge_workspace` | `attention.purge:<workspace>` | | that workspace |
| 9 | `search::purge_workspace` | `search.purge_workspace` | `search.purge:<workspace>` | | that workspace |
| 10 | `conversations::purge_workspace` | `conversations.purge_workspace` | `conversations.purge:<workspace>` | | that workspace |
| 11 | `workspaces::delete_workspace` | `workspaces.purge_workspace` | `<workspace>` | | that workspace |
| 12 | `search::backfill` | `search.backfill` | `search.backfill:<workspace>:<group type>:<page>` | | the workspace |
| 13 | `conversations::decide_message` (backlog) | `conversations.decide_backlog` | `conversations.decide_backlog:<message>` | | the message's workspace |
| 14 | `conversations::send_draft` | `conversations.release_send` | `release:<draft>:<generation>` | | the draft's workspace |
| 15 | `conversations::release_send` | `conversations.check_send` | `check:<send>` | | the draft's workspace |
| 16 | `conversations::apply_changes` | `conversations.apply_changes` | `<inbox>:<blob hash>` | `conversations.inbox:<inbox>` | the inbox's workspace |
| 17 | `conversations::backfill_needs` | `conversations.backfill_needs` | `conversations.backfill_needs:<inbox>:<after>` | | the inbox's workspace |
| 18 | `conversations::fill_templates` | `conversations.fill_templates` | `conversations.fill_templates:<inbox>:<page>` | | the inbox's workspace |
| 19 | `conversations::domain::rule_runs` | `conversations.apply_rule` | `conversations.apply_rule:<run>:<phase>:<after>` | | the run's workspace |

None relies on dedup or ordering **across** workspaces: every workspace
caller's key names a record of its own workspace (a stream, send, inbox,
message, draft, run, or the workspace itself), so two workspaces never
legitimately share one, and the instance-wide callers (1, and 2 for account
events) enqueue only instance-wide rows. Found by grepping every `.unique(`,
`.ordered_by(`, `unique_key`/`ordering_key` and `ON CONFLICT` in the crates
and migrations, and every raw `INSERT INTO kernel.jobs` (tests and the
modules' test helpers; none passes `ON CONFLICT`).

## Decisions

### `NULLS NOT DISTINCT` on `workspace_id`, not a partial pair or `workspace_key`

Three ways to key the unique index by workspace with the instance-wide rows
as their own group:

1. Two partial unique indexes, `(kind, unique_key, workspace_id) WHERE
   workspace_id IS NOT NULL` and `(kind, unique_key) WHERE workspace_id IS
   NULL`. An `ON CONFLICT` can infer only indexes on one column list, so the
   enqueue would need the target chosen per row (`replay` inserts both kinds
   in one statement), or a target-less `ON CONFLICT DO NOTHING` that also
   swallows any other unique violation.
2. `(kind, unique_key, workspace_key)`, the generated
   `coalesce(workspace_id, '')`. One index, any Postgres version, but a
   workspace id `''` (only the kernel scope could write one) would share the
   instance-wide group.
3. `(kind, unique_key, workspace_id) NULLS NOT DISTINCT`: one index, one
   target, exact semantics (a NULL workspace equals only NULL). Needs
   Postgres 15+.

Chosen: 3. Campbooks already requires Postgres 17 (`kernel.uuid_v7()` exists
to keep 17 supported until 18's `uuidv7()`; CI, the dev compose file and the
self-host compose file run `pgvector/pgvector:pg17`). `ON CONFLICT` infers a
`NULLS NOT DISTINCT` index and treats NULLs as equal there (checked on 17).
`kind, unique_key` lead, so the heal, the admin's discard and retry (which
look up by kind and key) still use the index.

### Ordering by `workspace_key`

The claim's check is a correlated `NOT EXISTS` on every candidate, so it
must stay an index lookup. `o.workspace_id IS NOT DISTINCT FROM
j.workspace_id` is not indexable; `o.workspace_key = j.workspace_key` is,
with `(ordering_key, workspace_key, seq)`. The `''` ambiguity is harmless
here: the only effect of a collision would be a job waiting longer, never
running out of order or not at all. The advisory lock uses
`jsonb_build_array(workspace, key)`, so no workspace and key pair can spell
another's lock string.

### The enqueue lock and its effect on enqueuers

The advisory lock serializes enqueuers of a key until they commit, so `seq`
order is commit order. Per workspace, an open transaction of workspace B
holding the key no longer makes A's enqueuers wait (a second way B could
stall A's request handlers). Hash collisions of `hashtextextended` stay
possible, as before: a collision only serializes two enqueuers.

### #207's guards

Unchanged and still needed: `workspace_isolation`'s `WITH CHECK` refuses a
`schedule:%` key on any workspace row, and `instance_rows_insert` admits only
listed fresh deliveries keyed by their event. The schedule tick's job is
instance-wide (`workspace_id` NULL), so its `ON CONFLICT` now dedupes only
against instance-wide rows: a forged workspace row with the key would no
longer even block it, and the heal still retires such a row. The existing
tests (`schedules_enqueue_each_tick_once_across_workers`,
`a_workspace_cannot_forge_instance_wide_jobs_or_events`,
`a_stuck_schedule_row_is_replaced_by_the_next_tick`) pass unchanged.

## Migration

| Version | File | Transaction | Lock |
|---|---|---|---|
| `20261021130000` | `kernel__job_unique_keys_per_workspace_index` | none (`-- no-transaction`, one statement) | `CREATE UNIQUE INDEX CONCURRENTLY`: `SHARE UPDATE EXCLUSIVE`, enqueues and claims go on |
| `20261021130100` | `kernel__job_ordering_keys_per_workspace_index` | none | same |
| `20261021130200` | `kernel__job_keys_per_workspace` | one, `lock_timeout = 5s` | `DROP INDEX` ×2: `ACCESS EXCLUSIVE` on `kernel.jobs` for a catalog change only (milliseconds) |

- The concurrent builds follow the conversations' list indexes: a failed or
  interrupted build leaves an INVALID index, which the server drops before
  migrating (`drop_invalid_indexes`, under the migration lock), and
  `IF NOT EXISTS` keeps a valid one built just before a crash.
- The new unique index cannot find duplicates: the old index already allowed
  at most one unfinished row per `(kind, unique_key)`.
- The old indexes go only after both new ones exist. If `130200` times out
  (a long transaction on `kernel.jobs`), the migration fails and is retried
  at the next start; both index sets exist meanwhile and either binary works.
- **Deploy order.** After `130200`, a binary from before this change cannot
  enqueue a job with a unique key: Postgres refuses `ON CONFLICT (kind,
  unique_key) WHERE …` when no index matches (42P10, checked on 17). It can
  still enqueue unkeyed jobs and claim. The new binary always migrates
  before it serves (migrations run at startup), and the hosted instance runs
  one container whose old process stops before the new one migrates, so
  there is no window. A self-hoster running several processes must restart
  them all on the new image, as for any migration.

## Risks

- **Background work that could stop:** an old process still running after
  the migration fails every keyed enqueue (event deliveries are unkeyed at
  publish and keep flowing; schedules, syncs, sends and purges do not). Only
  with mixed versions; see the deploy order.
- **More concurrency, not less:** two workspaces that used to share a key now
  run side by side. Every key names a record of one workspace, so no caller
  depended on that serialization (inventory above).
- The ordering index adds `workspace_key` (a short text) to each entry; the
  partial index covers unfinished jobs only.

## Rollback

sqlx has no down migrations, and refuses to start with applied migrations it
does not know. **Ops step: before redeploying an image older than this
change, run this once as the migration login.** It restores the exact
indexes of `20261012100000`, drops the per-workspace ones, and forgets this
change's migrations so a later upgrade applies them again. Checked against a
database migrated only to the base: identical `pg_indexes` for schema
`kernel` and identical `_sqlx_migrations`, and re-upgrading applies cleanly.

The old unique index cannot be built while two workspaces hold unfinished
jobs of one kind and key (the transaction then fails and changes nothing).
Check first:

```sql
SELECT kind, unique_key, count(*) FROM kernel.jobs
 WHERE unique_key IS NOT NULL AND state IN ('pending', 'running')
 GROUP BY kind, unique_key HAVING count(*) > 1;
```

Wait for those to finish, or set the later pending ones aside as failed
(kept for inspection; the admin can retry them after the rollback):

```sql
UPDATE kernel.jobs j
   SET state = 'failed', finished_at = now(),
       last_error = 'set aside to roll back campbooks-cloud#209', updated_at = now()
 WHERE j.state = 'pending' AND j.unique_key IS NOT NULL
   AND EXISTS (SELECT 1 FROM kernel.jobs o
                WHERE o.kind = j.kind AND o.unique_key = j.unique_key
                  AND o.state IN ('pending', 'running') AND o.seq < j.seq);
```

Then:

```sql
-- Roll back campbooks-cloud#209 BEFORE redeploying an image older than it.
-- Run once, as the migration login (the table's owner). Restores the exact
-- cross-workspace key indexes of 20261012100000, drops the per-workspace
-- ones, and forgets this change's migrations, so the older image starts and
-- a later upgrade applies them again.
BEGIN;
SET LOCAL lock_timeout = '5s';
CREATE UNIQUE INDEX jobs_unique_key_idx ON kernel.jobs (kind, unique_key)
    WHERE unique_key IS NOT NULL AND state IN ('pending', 'running');
CREATE INDEX jobs_ordering_idx ON kernel.jobs (ordering_key, seq)
    WHERE ordering_key IS NOT NULL AND state IN ('pending', 'running');
DROP INDEX kernel.jobs_unique_key_per_workspace_idx;
DROP INDEX kernel.jobs_ordering_per_workspace_idx;
DELETE FROM _sqlx_migrations WHERE version IN (20261021130000, 20261021130100, 20261021130200);
COMMIT;
```

The two `CREATE INDEX` scan `kernel.jobs` under a `SHARE` lock (writes wait;
well under a second on a queue pruned hourly). Every process must run the
same side of the change: upgrade (or roll back) the API and the worker
together.

## Left open

- The payloads of `identity.consent_revoked` / `identity.way_in_removed`
  appended from a workspace are not bound to it (no consumer today), and
  there is no admin UI for `DELETE /admin/v1/system/schedules/{name}/run`;
  both stay on campbooks-cloud#209's list, outside this change.
