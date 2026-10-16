# Design

## Context

Row-level security on the kernel's workspace-keyed tables (`kernel.events`,
`kernel.jobs`, `kernel.changes`, `kernel.change_heads`, `kernel.blobs`,
`kernel.blob_refs`; migration `20261020040000_kernel__workspace_isolation`)
admitted every row to a transaction that set no workspace, so the kernel's
machinery could work across workspaces. Module tables were already strict
(`workspace_id = <setting>`: an unset transaction compares with NULL and
sees nothing), except two maintenance policies keyed on "unset":
`connectors.sync_due` (`maintenance_reads_schedule`,
`maintenance_leases_schedule`) and `conversations.needs_backfills`
(`maintenance_reads_backfills`, `maintenance_starts_backfills`).

The precedent is ADR 0015's `campbooks.admin_reads`: a transaction-local
setting only the kernel names, set only by `begin_admin`.

## Decisions

### The kernel scope

`campbooks.kernel_scope = 'on'` (`kernel::db::KERNEL_SCOPE_SETTING`,
`SET LOCAL` through `set_config(…, true)`). The kernel tables' policies:

- `changes`, `change_heads`, `blobs`, `blob_refs`:
  `kernel_scope = 'on' OR workspace_id = <workspace>`.
- `events`, `jobs`: `kernel_scope = 'on' OR (workspace_id IS NULL AND
  <workspace> IS NOT NULL) OR workspace_id = <workspace>`.

A transaction with neither sees no row. A workspace transaction keeps
seeing the instance-wide rows (no workspace) of the event log and the
queue, as before: the queue's `ON CONFLICT` on a unique key, a workspace
job enqueueing an instance-wide follow-up and a workspace handler
publishing an instance-wide event all rely on it, and narrowing it is not
this change's goal.

The setting is checked against `'on'` (not merely set), like
`admin_reads`, so an empty value left by an earlier transaction on the same
connection reads as off.

### How code opens it

- `kernel::db::begin_kernel(pool)`: a transaction in the kernel scope.
- `JobContext::kernel_tx()`: the job's own transaction in the kernel scope,
  committed with the job's completion like `tx()`. It refuses a job that
  belongs to a workspace (`Fatal`): cross-workspace work is instance work.
- Inside the kernel only: `enter_kernel_scope` / `leave_kernel_scope` on an
  open transaction, `begin_in(pool, workspace)` (a workspace transaction for
  the kernel's text workspace ids), `scope()` / `require_scope()`.

Modules with maintenance policies get **the kernel helper plus a reviewed
allowlist** rather than either alone. A helper alone would let any module
call it; an allowlist alone would have them set the setting by string.
The policies trust the setting, so the arch rule is the whole guarantee.
`KERNEL_SCOPE_SETTING` is crate-private (tests that inspect policies ask
`kernel::testing::kernel_scope_setting()`). `cargo xtask arch` rejects,
outside the kernel and after stripping comments (not strings):
the setting's text `campbooks.kernel_scope` anywhere, tests included; the
identifiers `KERNEL_SCOPE_SETTING` and `ADMIN_READS_SETTING` in any use; and
the identifiers `begin_kernel` and `kernel_tx` in any use (a call, a
`use … as` rename, `JobContext::kernel_tx(ctx)`, a function value) unless
the file is listed under `[[access.kernel_scope]]` in `architecture.toml`
with a reason. An entry naming a file that no longer uses either is itself
a violation, so exceptions do not outlive their reason. Test code is exempt
from the identifier rule (not from the setting's text). A capability token like `AdminGrant` was
considered: nothing natural mints it for a scheduler (the worker would have
to hand it to every instance-wide job), so it would protect less than the
allowlist and read worse.

### Instance-wide rows from a transaction that set nothing

Identity's requests and jobs (accounts are instance-wide) publish events and
enqueue jobs with no workspace from transactions that set none, and so does
the worker's schedule tick. Under the new policies such a transaction can
neither insert nor read back (`RETURNING`) those rows. Two options:

1. An `INSERT`-only policy for instance-wide rows. `RETURNING` (the publish
   CTE returns the event id; enqueue returns the job id) needs the row to
   pass the `SELECT` policies too, so every insert would have to be
   rewritten with client-side ids and no `RETURNING`.
2. **Chosen:** the kernel lifts the transaction into its scope for its own
   insert and lowers it right after (`lift_for_instance_rows` / `lower`),
   only when the transaction set nothing and the row is instance-wide (its
   workspace parameter is `None`). No caller code runs while it is lifted,
   and the statement writes `NULL` workspaces only. On an error the
   transaction is aborted anyway.

   The invariant, documented on `lift_for_instance_rows`: it lifts only a
   fully unset transaction; `lower` turns the scope off only if it lifted
   (a debug assertion checks the transaction is unset again); and a
   publish or enqueue future dropped mid-statement would leave the scope on,
   so a caller that loses one must drop its transaction too, never keep
   using it. Every caller awaits them to completion.

The worker completes a handler's work in the handler's transaction; for an
instance-wide job that transaction set no workspace and would no longer see
its own job row, so the worker turns the kernel scope on for that last
`UPDATE` (the handler has returned).

### Refusals

Like `EventBus::publish` (which keeps its rule: a workspace's event only in
that workspace's transaction), these return `sqlx::Error::InvalidArgument`
when the transaction cannot reach the workspace's rows (set none and no
kernel scope, or set another workspace), instead of silently seeing and
changing nothing:

- `jobs::enqueue` / `enqueue_to` (and `JobContext::enqueue*`) with a
  workspace;
- `changes::record` (called by `publish` for events declaring changes);
- `BlobStore::attach`, `detach`, `detach_owner`, `detach_owners`
  (`BlobError::Database`);
- `jobs::purge_workspace` and `changes::purge_workspace`.

### Every path, and its scope

Found by reading every query on the six kernel tables and every
`begin()`/`acquire()`/pool executor in the kernel, the server and the
modules, then running the whole workspace's tests (most module test kits
run as the runtime role, so a missed path fails them).

| # | Path | Where | Scope now (before) |
|---|------|-------|--------------------|
| 1 | Worker claim (fair share across workspaces) | `kernel::jobs::worker::claim` | kernel scope, own transaction (pool) |
| 2 | Lease renewal, release on shutdown, failure record, reaping expired leases | `worker::{heartbeat, release, fail, reap}` | kernel scope (pool) |
| 3 | Queue depth metrics | `worker::sample_queues` | kernel scope (pool) |
| 4 | Schedule tick (instance-wide job) | `worker::tick` | kernel scope (unset) |
| 5 | Completion with no handler transaction | `worker::mark_succeeded_on` | kernel scope (pool) |
| 6 | Completion in the handler's transaction | `worker::mark_in` | the job's workspace; an instance-wide job's transaction lifted for the final `UPDATE` (unset) |
| 7 | Event delivery: loading the event (the dispatcher) | `jobs::registry::load_event` | the delivery's workspace; kernel scope for an instance-wide event, matching `workspace_id IS NOT DISTINCT FROM` the delivery's (pool) |
| 8 | Fan-out at publish (event + deliveries) | `EventBus::publish` | publisher's workspace; instance-wide event from an unset transaction: lifted for the insert (unset) |
| 9 | Enqueue | `jobs::insert` | caller's workspace (refused otherwise); instance-wide job from an unset transaction: lifted (unset) |
| 10 | Unsettled check (`member_left` before re-joining) | `EventBus::unsettled` | caller's workspace transaction (unchanged) |
| 11 | Replay | `EventBus::replay` | kernel scope (unset) |
| 12 | Change recording | `changes::record` | publisher's workspace (refused otherwise) |
| 13 | Change sequencer | `changes::SequenceChanges` | its job's workspace (unchanged) |
| 14 | Change feed reader (head, pages) | `changes::stream::{open, read_page}` | the subscription's workspace, one short transaction per read (pool) |
| 15 | Feed retention and re-sequencing stranded changes | `changes::PruneChanges` | `kernel_tx` (unset) |
| 16 | Job retention | `jobs::PruneJobs` | kernel scope per batch (pool) |
| 17 | Event-log retention | `jobs::PruneEvents` | kernel scope per batch (pool) |
| 18 | Blob catalogue upsert on put | `blobs::Inner::touch` | the blob's workspace (pool) |
| 19 | Blob sweeper | `BlobStore::sweep` | kernel scope per batch (unset) |
| 20 | Blob purge of a deleted workspace | `BlobStore::purge_workspace` | that workspace (unset) |
| 21 | Attach / detach | `BlobStore::{attach, detach, detach_owner, detach_owners}` | caller's workspace (refused otherwise) |
| 22 | Workspace purge of jobs, events and the feed | `jobs::purge_workspace`, `changes::purge_workspace` (from `workspaces.purge_workspace`) | the purge job's workspace (unchanged; refused otherwise) |
| 23 | Admin queue: summary, failed, retry, retry kind, discard | `kernel::jobs::admin` (system module's admin operations) | kernel scope, behind the `AdminGrant` (pool / unset) |
| 24 | Connectors dispatcher (`sync_due` lease + enqueue per workspace) | `connectors::domain::schedule::dispatch` | `begin_kernel`, allowlisted (unset) |
| 25 | Needs backfill dispatcher | `conversations::features::backfill_needs::DispatchNeedsBackfills` | `kernel_tx`, allowlisted (unset) |
| 26 | Identity's requests and jobs publishing account events | `crates/modules/identity/**` | unset, as before; their instance-wide rows are lifted by the kernel (row 8, 9) |
| 27 | Identity's unnamed-mailbox sweep | `identity::prune::sweep_unnamed_mailboxes` | unset for identity's own tables, its announcements lifted; the connection lookups go through the connectors port in their workspace |
| 28 | Admin overviews of module tables | connectors, workspaces, system admin reads | `begin_admin` (unchanged; touch no kernel table) |
| 29 | Startup checks | migration status (`_sqlx_migrations`), role inspection (catalogue), invalid indexes (migration login) | unchanged: no table with row-level security |

The kernel's tables without a workspace (`kernel.idempotency`,
`kernel.rate_limits`, `kernel.schedules`) and the BFF's sessions carry no
policy and are unchanged.

### Arch-rule gaps

Closed in `cargo xtask arch` (`access::check_workspace_queries`,
`access::check_kernel_scope`):

- `.acquire()`, `Acquire::begin(` and `Acquire::acquire(` count as opening a
  connection without a workspace, like `.begin()`.
- SQL kept in a `const NAME: &str` of the same file is resolved when a
  query names the constant.
- The server and the kernel are checked as well as the modules, with
  reviewed `pool_access` entries: the BFF (instance-wide sessions), the
  kernel's `db.rs` (where `begin_for` / `begin_admin` / `begin_kernel` open
  transactions) and `jobs/context.rs` (a job's transaction).
- The kernel scope's setting, its constant and its openers, by identifier
  (any use, not only a call), and the admin reads' constant likewise, as
  above.

Left open, and why:

- **Executors the regex does not name.** A query run on a pool held under
  another name (`let p = state.db.clone(); … .fetch_all(&p)`), or a pool
  passed through a generic `impl PgExecutor`, is not recognised. Following
  values needs type information the text check does not have. Row-level
  security is the backstop: such a query now sees nothing.
- **SQL built at runtime** (`format!`, `AssertSqlSafe`, a `const` from
  another file) is not resolved, for the same reason.
- **Tests** are exempt (fixtures set up rows across workspaces on purpose).

## Risks

- **Background work that silently stops.** A cross-workspace path missed
  here would now find no rows rather than fail. Mitigation: the table
  above, the whole-workspace test run as the runtime role, and the
  maintenance-path tests (claim and complete across workspaces, deliveries
  of workspace and instance-wide events, schedules, pruning, sweep, replay,
  admin queue, the two module dispatchers) under the runtime role.
- **Deploy order.** The migrations and the code ship together; a process
  still running the previous build after the migration has run (a rolling
  restart) would stop claiming jobs until it is replaced. The server
  migrates on start, so a single-instance restart has no such window.
- **Extra round trips.** The worker's bookkeeping statements now open a
  transaction each (`BEGIN`, the setting, the statement, `COMMIT`).

## Rollback

sqlx has no down migrations, and an image older than this change breaks
once the migrations have run: identity publishes account events from
transactions that set nothing without the kernel's lift (its
`INSERT INTO kernel.events` fails row-level security, so sign-in fails),
and the old worker claims nothing. **Ops step: before redeploying an older
tag, run this once as the migration login.** It restores the exact policy
expressions of `20261020040000_kernel__workspace_isolation`,
`20261020040100_connectors__schedule_policies` and
`20261020050000_conversations__needs_backfill`, and forgets the three
migrations so the older image starts and a later upgrade applies them
again. Checked against a database migrated only to the base: identical
`pg_policies` and `_sqlx_migrations`, and re-upgrading applies cleanly.

```sql
-- Roll back campbooks-cloud#129 (kernel scope) BEFORE redeploying an image
-- older than it. Run once, as the migration login (the tables' owner).
-- Restores the exact policies of 20261020040000, 20261020040100 and
-- 20261020050000 and forgets the three migrations, so the older image
-- starts (sqlx refuses applied migrations it does not know) and a later
-- upgrade applies them again.
BEGIN;
SET LOCAL lock_timeout = '5s';
DO $$
DECLARE
    t text;
BEGIN
    FOREACH t IN ARRAY ARRAY['changes', 'change_heads', 'blobs', 'blob_refs']
    LOOP
        EXECUTE format('DROP POLICY workspace_isolation ON kernel.%I', t);
        EXECUTE format(
            'CREATE POLICY workspace_isolation ON kernel.%I USING '
            '(nullif(current_setting(''campbooks.workspace_id'', true), '''') IS NULL '
            ' OR workspace_id = nullif(current_setting(''campbooks.workspace_id'', true), ''''))',
            t);
    END LOOP;
    FOREACH t IN ARRAY ARRAY['events', 'jobs']
    LOOP
        EXECUTE format('DROP POLICY workspace_isolation ON kernel.%I', t);
        EXECUTE format(
            'CREATE POLICY workspace_isolation ON kernel.%I USING '
            '(nullif(current_setting(''campbooks.workspace_id'', true), '''') IS NULL '
            ' OR workspace_id IS NULL '
            ' OR workspace_id = nullif(current_setting(''campbooks.workspace_id'', true), ''''))',
            t);
    END LOOP;
END
$$;
DROP POLICY maintenance_reads_schedule ON connectors.sync_due;
DROP POLICY maintenance_leases_schedule ON connectors.sync_due;
CREATE POLICY maintenance_reads_schedule ON connectors.sync_due
    FOR SELECT
    USING (nullif(current_setting('campbooks.workspace_id', true), '') IS NULL);
CREATE POLICY maintenance_leases_schedule ON connectors.sync_due
    FOR UPDATE
    USING (nullif(current_setting('campbooks.workspace_id', true), '') IS NULL)
    WITH CHECK (nullif(current_setting('campbooks.workspace_id', true), '') IS NULL);
DROP POLICY maintenance_reads_backfills ON conversations.needs_backfills;
DROP POLICY maintenance_starts_backfills ON conversations.needs_backfills;
CREATE POLICY maintenance_reads_backfills ON conversations.needs_backfills FOR SELECT
    USING (nullif(current_setting('campbooks.workspace_id', true), '') IS NULL);
CREATE POLICY maintenance_starts_backfills ON conversations.needs_backfills FOR UPDATE
    USING (nullif(current_setting('campbooks.workspace_id', true), '') IS NULL)
    WITH CHECK (nullif(current_setting('campbooks.workspace_id', true), '') IS NULL
                AND state = 'running');
DELETE FROM _sqlx_migrations
 WHERE version IN (20261020200000, 20261020200100, 20261020200200);
COMMIT;
```

Every process must run the same side of the change: upgrade (or roll back)
the API and the worker together.
