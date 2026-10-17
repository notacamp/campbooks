# Design

## Context

After campbooks-cloud#196 (archived change `2026-10-17-kernel-scope-follow-ups`,
migration `20261021010000_kernel__instance_rows_read_insert_only`), the
policies on `kernel.events` and `kernel.jobs` were:

| Policy | Command | Expression |
|---|---|---|
| `workspace_isolation` | `ALL` | `kernel_scope = 'on' OR workspace_id = <workspace>` |
| `instance_rows_read` | `SELECT` | `workspace_id IS NULL AND <workspace> IS NOT NULL` |
| `instance_rows_insert` | `INSERT` | `workspace_id IS NULL AND <workspace> IS NOT NULL` |

`instance_rows_insert` admitted any instance-wide row from any workspace
transaction, and `workspace_isolation` admitted any row of the workspace's
own, whatever its unique key. The unique index is `(kind, unique_key) WHERE
state IN ('pending', 'running')`, across workspaces, and the schedule tick
enqueues with `unique_key = 'schedule:<name>'` and `ON CONFLICT DO NOTHING`:
one forged row with that key and `run_at = 'infinity'` stopped the schedule
for good.

Data ownership is unchanged: the kernel owns schema `kernel`, including the
new `kernel.instance_rows_from_workspaces`. No event is published or
consumed by this change and no AI is used. The one new endpoint is an admin
operation (`admin:access` bearer scope, `jobs.retry` admin permission).

## Every legitimate instance-wide insert from a workspace transaction

Found by reading every `publish` of an event whose `workspace()` can be
`None`, every `enqueue`/`enqueue_to` (direct and `ctx.enqueue`) and every
raw `INSERT INTO kernel.jobs/events` in the kernel, the server and the
modules, then checked by running the whole workspace's tests with the list
emptied (the paths below then fail; see Tests).

| # | Path | Transaction | Inserts |
|---|------|-------------|---------|
| 1 | `identity::features::remove_client` (a member removes a workspace client) | `begin_for(workspace)` | `identity.consent_revoked` events |
| 2 | `identity.revoke_departed_consents` ← `workspaces.member_left`, incl. the departed member's clients | the delivery's workspace (`ctx.tx()`) | `identity.consent_revoked` events |
| 3 | `identity.forget_connected_way_in` ← `connectors.connection_removed` | the delivery's workspace | `identity.way_in_removed` event |
| 4 | Deliveries of 1–3 | same statement as the event | none today: neither event has a consumer |

Not from a workspace transaction, so not affected:

- every other identity publish (sign-in, sign-up, passkeys, ways in,
  consents, token reuse, suspension, sign-out everywhere, the unnamed
  mailbox prune): `pool.begin()`, which set nothing, lifted by
  `lift_for_instance_rows` for the insert only;
- the schedule tick, pruning, replay, the admin queue: the kernel scope;
- the dispatchers (maintenance scope, no workspace): they enqueue workspace
  jobs only;
- every module `enqueue`: all name a workspace (`for_workspace`, or the
  job's own via `ctx.enqueue`), purges and `run_at` far ahead included;
- `changes::request_sequencing`: a workspace job;
- the server: only tests enqueue instance-wide jobs, from unset
  transactions.

No production path enqueues an instance-wide job from a workspace
transaction. The kernel test that did (an "instance-wide follow-up") now
enqueues from an unset transaction.

## Threat model

These settings (`campbooks.workspace_id`, `kernel_scope`,
`maintenance_scope`) are defence in depth against **code mistakes**, not
against arbitrary SQL: any SQL the runtime role runs can `set_config` them,
and the arch rule's word check still misses names split below the word
(`concat!("campbooks.kernel", "_scope")`). A module that writes raw SQL
into the kernel's tables by mistake is stopped; an attacker with SQL
injection is not. The real fix for the latter is separate database roles
per scope (the kernel's machinery on its own login), a separate change.

## Decisions

### The allowlist: a kernel table, written by migration, checked by a test

Options considered:

1. **Hard-code the names in the policy** and keep them in sync with an
   `xtask` check. `xtask` cannot build the registry (it would have to
   compile and run the server), so it would parse Rust for consumers; and
   every addition re-creates the policy.
2. **A kernel table the server fills at startup from the registry.** No
   migration per addition, but the runtime role would need to write the
   table that limits it (row-level security on it would again rest on the
   kernel scope), two builds during a deploy would overwrite each other's
   list, and every test kit would have to fill it.
3. **A kernel table written by migration, read by the policies, compared
   with the registry by a server test** (chosen). The runtime role has
   `SELECT` only, so no runtime path, the kernel's included, can widen what
   a workspace may add. The server's test
   (`the_instance_rows_a_workspace_may_add_match_the_registry`) has the
   real registry and fails, naming the difference, when a new declared
   event or a new consumer of one lacks its row. Adding one is a two-line
   kernel migration, reviewed like any other.

What the registry derives (`Registry::instance_rows_from_workspaces`): the
events declared `DomainEvent::PUBLISHED_IN_WORKSPACES = true` (a new
constant, default `false`) and the names of their consumers (the delivery
job kinds). A declaration is the publishing module's statement that a
workspace handler announces this event; its doc says a consumer must not
take such an event as proof of who published it, since any workspace
transaction may now append one.

### The policies

| Policy | Command | Expression |
|---|---|---|
| `workspace_isolation` (jobs) | `ALL` | `USING` as before; `WITH CHECK`: `kernel_scope = 'on' OR (workspace_id = <workspace> AND (unique_key IS NULL OR unique_key NOT LIKE 'schedule:%'))` |
| `workspace_isolation` (events) | `ALL` | unchanged |
| `instance_rows_read` | `SELECT` | unchanged |
| `instance_rows_insert` (jobs) | `INSERT` | `workspace_id IS NULL AND <workspace> IS NOT NULL AND state = 'pending' AND attempts = 0 AND locked_by IS NULL AND locked_until IS NULL AND run_at <= now() AND payload = '{}' AND event_id IS NOT NULL AND unique_key = event_id::text AND unique_key NOT LIKE 'schedule:%' AND ordering_key IS NULL AND kind` listed as `'job'` |
| `instance_rows_insert` (events) | `INSERT` | `workspace_id IS NULL AND <workspace> IS NOT NULL AND occurred_at <= now() AND name` listed as `'event'` |

- The job check is exactly the shape `EventBus::publish` writes a delivery
  in (`run_at`, `occurred_at` default to `now()`, the transaction's start,
  so `<= now()` admits them and nothing later): a forged delivery can only
  be a fresh one, for a listed consumer, of an event id. Its consumer then
  loads the event and refuses one of another name (`load_event`), so the
  worst a workspace can do is deliver a listed event again, or a forged
  listed event (see Non-goals).
- `unique_key NOT LIKE 'schedule:%'` is implied by `unique_key =
  event_id::text`; it is written out so that loosening the delivery shape
  later cannot reopen the schedule.
- The schedule ban is in `workspace_isolation`'s `WITH CHECK` too, because
  the unique index spans workspaces: a workspace's own row with a
  schedule's key blocked the tick as well. Workspace UPDATEs (the worker
  marking a workspace job done in its transaction) keep passing: no
  workspace job carries such a key.
- The kernel scope passes `workspace_isolation` and is untouched: the tick,
  the heal, pruning and the admin queue.
- A listed delivery must also have `started_at`, `finished_at` and
  `last_error` empty and be due no more than five minutes ago; a listed
  event must be dated within the last five minutes. Its queue is not
  pinned: consumers choose their own.
- A delivery is **not** checked against its event's row (`EXISTS (SELECT
  … FROM kernel.events …)`), although the review suggested it:
  `EventBus::publish` inserts the event and its deliveries in one statement
  (a data-modifying `WITH`), and a policy subquery runs on that statement's
  snapshot, which does not see the event being inserted (checked: such a
  policy refuses every publish with a consumer). `load_event` covers the
  case instead (B2).

The kernel refuses the same things first, with a clear message
(`sqlx::Error::InvalidArgument`): `lift_for_instance_rows` now takes what
it writes and whether workspaces may add it, and refuses an instance-wide
job, or an instance-wide event not declared `PUBLISHED_IN_WORKSPACES`, from
a transaction that works in a workspace outside the kernel scope.

### Healing a stuck schedule row

`Worker::tick`, in its kernel-scope transaction, after the tick advanced and
before it enqueues, marks failed every pending or running row of
`kind = <name>, unique_key = 'schedule:<name>'` that

- belongs to a workspace (only a forgery from before this change), or
- is pending and due later than `2 × max(every, retry_max_seconds)` from
  now (no tick or retry puts it there), or
- is running on a lease ending later than `2 × lease_seconds` from now (no
  worker leases that long),

with `last_error = 'stuck schedule row replaced by the schedule's tick'`
and a warning log, then enqueues the tick as before. A failed row leaves
the unique index, is listed in the Jobs panel and is pruned with the failed
jobs. A row waiting out a retry, or held by a live worker, is left alone.
The bound assumes a scheduled job's own `retry_after` stays within
`CAMPBOOKS_JOBS__RETRY_MAX_SECONDS` (none asks for more today; one that did
would be replaced at the next tick), and shrinking `LEASE_SECONDS` during a
rolling deploy can make a new worker take a running schedule row leased by
an old one for stuck, which may run that schedule's job twice once.
It runs once per tick period (hourly for the kernel's prunes, every minute
for the dispatchers).

`jobs::admin::discard_schedule(state, grant, name)` (operation
`system.admin_discard_schedule_run`, `DELETE
/admin/v1/system/schedules/{name}/run`, permission `jobs.retry`, audited
like the other admin commands) deletes the schedule's unfinished row when
it is pending (any due time), belongs to a workspace, or runs on a lease
that expired or is longer than `2 × lease_seconds`; a run a live worker
holds is kept. Unknown names are 404. It is by name, not by id, because the
admin API lists only failed jobs' ids.

### The schedules' last ticks (review B1)

`kernel.schedules` (one row per schedule: its last enqueued tick) had no
row-level security and the runtime role could write it, so any transaction
could set `last_tick` to the largest bigint: every later tick's upsert
(`WHERE last_tick < excluded.last_tick`) then returned no row, and the tick
returned before the heal. Every schedule could stop (pruning, the stream
dispatcher, change sequencing, session pruning, the needs backfills), and
the admin discard could not help (there was no row to discard).

- The migration enables and forces row-level security on it, with one
  `kernel_scope_only` policy (`FOR ALL`, `USING` and `WITH CHECK`
  `kernel_scope = 'on'`). Its only caller, `Worker::tick`, already runs in
  `begin_kernel`; nothing else reads or writes it (tests included).
- Backstop: the tick's upsert also takes over a `last_tick` more than one
  period ahead (`OR last_tick > excluded.last_tick + 1`), so a clock that
  jumped, or a row written before the policy, heals on the next tick.
- The server's policy test admits the kernel scope on `kernel.schedules`
  only through `kernel_scope_only`.

### An event is handled only in its own workspace (review B2)

`load_event` checked the event's name and version, and that the event row
and the delivery share a workspace, but not that the row's workspace is the
event's own. A workspace could write, into its own workspace (which
`workspace_isolation` admits), an `identity.signed_out_everywhere` naming
any account and a `bff.end_signed_out_sessions` delivery of it; the worker
ran the handler, which ends that account's web sessions across the instance
(`identity.notify_way_in_added` would send a security email with chosen
text). Now `load_event` fails the delivery for good (`Fatal`, the handler
never runs) unless the row's workspace equals the decoded event's own
`workspace()`, and an instance-wide type (`WORKSPACE_SCOPED = false`) is
never logged in a workspace. This also refuses a workspace's event whose
payload names another workspace than the row (a handler that trusts the
payload's workspace, on tables that are not workspace-isolated, would
otherwise act there). `EventBus::publish` and replay always write the
event's own workspace, so no legitimate delivery is refused. Both
consumer paths (`on_event`, `consumer`) go through `load_event`; nothing
else hands an event to a handler.

### The dispatchers clear the workspace (item 2)

`jobs::enqueue_in(tx, workspace, job, options)` and `enqueue_to_in` refuse
a transaction that already works in a workspace, set `workspace`, enqueue
with `options.workspace = workspace`, and clear the setting again (`''`,
which every policy reads as unset), also when the enqueue failed without
aborting the transaction. Both dispatchers (`connectors::domain::schedule`
`enqueue_due`, `conversations` `backfill_needs::queue_pages`) use it; lease
and enqueues stay in one transaction as before.

### The server's policy test (item 3)

`maintenance_policies_only_read_and_lease` now reads every policy outside
`pg_catalog` (the kernel's and the admin module's included), as
`qual || ' ' || with_check`, and matches a setting by its quoted name, not
by its comparison. It asserts: no policy admits a transaction for setting
no workspace; the kernel scope appears only in `kernel.*`
`workspace_isolation` (`ALL`) policies; the maintenance scope appears in
exactly the four maintenance policies of `connectors.sync_due` and
`conversations.needs_backfills`.

### Arch rules (items 4, 5)

In `xtask/src/access.rs`:

- `decoded(code)`: comments dropped, every non-raw string and character
  literal unescaped (`\x..`, `\u{..}`, `\n`, `\t`, `\r`, `\0`, `\\`, quotes,
  and a `\` line continuation with the next line's leading whitespace).
  `names_a_setting` matches each setting against the text as written and
  as decoded, and each bare word against the decoded text. Raw strings
  have no escapes and stay as written.
- `SCOPE_SETTINGS` gains `campbooks.admin_reads` / `admin_reads` with no
  reviewed list (only `begin_admin` sets it); the message names the
  opener of each setting.
- `without_tests` cuts at a `#[cfg(test)]` (more attributes allowed) `mod
  tests {` only at bracket depth 0 (all of `{[(`, in code only) and only
  when nothing but whitespace (comments are blanked) follows the module's
  closing brace. A nested test module, one inside a `macro_rules!` body (in
  braces or parentheses) or one followed by code no longer ends the check.
  One file had a `#[cfg(test)] mod usage_tests;` after its test module; it
  moved before it.

### The BFF (item 6)

`store.rs` gains `begin_refresh(pool)` (the refresh's transaction) and
`end_session(pool, id)` (a session deleted on the pool); `mod.rs` calls
those and its `pool_access` entry is gone, so `mod.rs` is checked like any
new BFF file. `store.rs` stays the BFF's one exempt file; its reason names
the refresh. No new exempt file was needed.

### Left open

- **Cross-workspace key squatting.** The unique index and ordering keys
  span workspaces; a workspace that knows another's key (most embed an id:
  `connectors.sync_stream:<stream>`, `assistant.purge:<workspace>`) can
  hold it with a row of its own that never runs, or order behind it. Fix:
  key the unique index and the ordering check by `workspace_key`; that is a
  queue-wide index change (a concurrent index build outside a migration
  transaction) and belongs in its own change.
- **Payloads of listed events** are not checked (see the proposal).
- **`kernel.idempotency` and `kernel.rate_limits`** have no row-level
  security either; they are filed separately.
- **The text checks** keep the limits listed in the previous change (names
  split below the word, executors under other names, macros).

## Risks

- **Background work that stops.** A missed path that adds an instance-wide
  row from a workspace transaction would now fail its transaction (and its
  job would retry, then fail) instead of succeeding. Mitigation: the path
  table, the kernel's own refusal with a message naming what to declare,
  the whole workspace's tests (identity's flows run as the runtime role),
  and the server's sync test. Tasks that could be hit: identity's consent
  and way-in announcements (listed); anything that would newly publish an
  instance-wide event from a workspace handler (must declare it).
- **The heal marking a legitimate row failed.** Only beyond twice the
  longest wait or lease; a handler's own `retry_after` longer than
  `2 × max(every, retry_max_seconds)` on a schedule's job would be replaced
  by the next tick. No schedule's job asks for that today.
- **Deploy order.** The migration and the code ship together. A process of
  the previous build still running after the migration keeps working: it
  never adds the refused rows (it would only now get an RLS error instead
  of a success for an instance-wide follow-up from a workspace, which no
  production path does).
- **Lock timeout.** The DROP/CREATE takes `ACCESS EXCLUSIVE` on
  `kernel.jobs` and `kernel.events`; `lock_timeout = 5s` fails the migration
  (and the start) on a busy queue instead of queueing every job statement
  behind it; the next start tries again.

## Rollback

sqlx has no down migrations, and refuses to start with applied migrations
it does not know. **Ops step: before redeploying an image older than this
change, run this once as the migration login.** It restores the exact
policies of `20261021010000_kernel__instance_rows_read_insert_only`, drops
the list, takes row-level security off `kernel.schedules` again, and forgets this change's migration so a later upgrade applies it
again. Checked against a database migrated only to the base: identical
`pg_policies`, tables and `_sqlx_migrations`, and re-upgrading applies
cleanly.

```sql
-- Roll back campbooks-cloud#207 BEFORE redeploying an image older than it.
-- Run once, as the migration login (the tables' owner). Restores the exact
-- policies of 20261021010000 (kernel.events, kernel.jobs), drops the list of
-- instance-wide rows workspaces may add, takes row-level security off
-- kernel.schedules again, and forgets this change's
-- migration, so the older image starts and a later upgrade applies it again.
BEGIN;
SET LOCAL lock_timeout = '5s';
DROP POLICY workspace_isolation ON kernel.jobs;
CREATE POLICY workspace_isolation ON kernel.jobs
    USING (current_setting('campbooks.kernel_scope', true) = 'on'
           OR workspace_id = nullif(current_setting('campbooks.workspace_id', true), ''));
DROP POLICY instance_rows_insert ON kernel.jobs;
CREATE POLICY instance_rows_insert ON kernel.jobs FOR INSERT
    WITH CHECK (workspace_id IS NULL
                AND nullif(current_setting('campbooks.workspace_id', true), '') IS NOT NULL);
DROP POLICY instance_rows_insert ON kernel.events;
CREATE POLICY instance_rows_insert ON kernel.events FOR INSERT
    WITH CHECK (workspace_id IS NULL
                AND nullif(current_setting('campbooks.workspace_id', true), '') IS NOT NULL);
DROP TABLE kernel.instance_rows_from_workspaces;
DROP POLICY kernel_scope_only ON kernel.schedules;
ALTER TABLE kernel.schedules NO FORCE ROW LEVEL SECURITY;
ALTER TABLE kernel.schedules DISABLE ROW LEVEL SECURITY;
DELETE FROM _sqlx_migrations WHERE version = 20261021040000;
COMMIT;
```

Every process must run the same side of the change: upgrade (or roll back)
the API and the worker together. Rows the heal marked failed stay failed
(they never ran as the schedule's); the older image prunes them with the
failed jobs.
