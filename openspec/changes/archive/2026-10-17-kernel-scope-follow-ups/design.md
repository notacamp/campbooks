# Design

## Context

After campbooks-cloud#129 (archived change `2026-10-16-kernel-scope`), the
policies on `kernel.events` and `kernel.jobs` were one `FOR ALL` policy:

```
workspace_isolation USING (
      kernel_scope = 'on'
   OR (workspace_id IS NULL AND <workspace> IS NOT NULL)
   OR workspace_id = <workspace>)
```

The middle branch let a workspace transaction see the instance-wide rows,
which the queue's unique-key `ON CONFLICT`, instance-wide follow-ups and
`RETURNING` need. Because the policy is `FOR ALL`, it also let it UPDATE,
DELETE and lock them. The two module dispatchers opened the full kernel
scope, which reaches every kernel table across workspaces, to read their
own index. The review of #512 also found deliberate ways around the arch
rule's text checks.

Data ownership is unchanged: the kernel owns schema `kernel`, connectors
owns `connectors.sync_due`, conversations owns
`conversations.needs_backfills`. No event is published or consumed by this
change, no endpoint and no AI use is added.

## Decisions

### Split the policy by command

Postgres combines permissive policies per command with OR. The new policies
on `kernel.events` and `kernel.jobs`:

| Policy | Command | Expression |
|---|---|---|
| `workspace_isolation` | `ALL` | `kernel_scope = 'on' OR workspace_id = <workspace>` |
| `instance_rows_read` | `SELECT` | `workspace_id IS NULL AND <workspace> IS NOT NULL` |
| `instance_rows_insert` | `INSERT` (`WITH CHECK`) | `workspace_id IS NULL AND <workspace> IS NOT NULL` |

- SELECT: either of the first two: as before.
- INSERT: either `WITH CHECK` (the first one's `USING` doubles as its
  check): as before. `RETURNING` and `ON CONFLICT` also read the row, which
  `instance_rows_read` admits.
- UPDATE, DELETE and row locks (`FOR UPDATE`, which Postgres checks against
  the UPDATE policies): only `workspace_isolation`. An instance-wide row is
  filtered out (zero rows, no error), and an UPDATE that would turn a
  workspace's row into an instance-wide one fails its `WITH CHECK`.
- A transaction that set nothing still sees nothing; the kernel's lift
  (`lift_for_instance_rows`) still covers its instance-wide inserts.

Keeping `workspace_isolation` as the one `FOR ALL` policy (rather than four
per-command ones) keeps the same expression as the four other kernel
tables' policy and the fewest policies to read.

### Every path that changes an instance-wide row, and where it runs

Found by reading every `UPDATE`/`DELETE`/`FOR UPDATE`/`ON CONFLICT` on
`kernel.jobs` and `kernel.events` in the kernel, the server and the modules,
then running the whole workspace's tests (most module kits run as the
runtime role). None ran in a workspace transaction, so none had to move; the
list is what keeps that true.

| # | Path | Where | Scope |
|---|------|-------|-------|
| 1 | Claim (`UPDATE … FROM picked`, `FOR UPDATE SKIP LOCKED`) | `jobs::worker::claim` | kernel scope |
| 2 | Lease renewal | `worker::heartbeat` | kernel scope |
| 3 | Release on shutdown | `worker::release` | kernel scope |
| 4 | Failure and retry record | `worker::fail` | kernel scope |
| 5 | Reaping expired leases (`FOR UPDATE SKIP LOCKED`) | `worker::reap` | kernel scope |
| 6 | Completion with no handler transaction | `worker::mark_succeeded_on` | kernel scope |
| 7 | Completion in the handler's transaction | `worker::mark_in` | an instance-wide job: the kernel scope is turned on before its `UPDATE` (the handler has returned); a workspace job's row is the workspace's own |
| 8 | Inserts from a transaction that set nothing | `db::lift_for_instance_rows` (publish, enqueue) | INSERT only, lifted into the kernel scope; from a workspace transaction the insert passes `instance_rows_insert` and its `RETURNING` passes `instance_rows_read` |
| 9 | Unique-key dedup | `jobs::insert_row`, `EventBus::replay` | `ON CONFLICT … DO NOTHING`: no UPDATE of the existing row, so no UPDATE policy is checked; the unique index sees the conflict whatever row-level security shows. Replay runs in the kernel scope |
| 10 | Ordering key, sequencing request | `jobs::insert_row`, `changes::request_sequencing` | reads only (`NOT EXISTS`), workspace rows |
| 11 | Schedule tick | `worker::tick` | kernel scope |
| 12 | Job and event retention | `jobs::PruneJobs`, `jobs::PruneEvents` | kernel scope, per batch |
| 13 | Admin queue: retry, retry kind, discard | `jobs::admin` | kernel scope |
| 14 | Workspace purge | `jobs::purge_workspace` | `WHERE workspace_id = $1`: the workspace's rows only |
| 15 | Modules and the server | | no `UPDATE`/`DELETE` of `kernel.jobs` or `kernel.events` outside tests (which run as the owner) |

Tests (`jobs::tests::kernel_scope`, as the runtime role):
`a_workspace_transaction_reads_and_adds_instance_rows_but_never_changes_them`
(UPDATE, DELETE and `FOR UPDATE` reach nothing; INSERT, `RETURNING`, a
unique-key duplicate and an instance-wide publish still work; the kernel
scope still changes them) and
`instance_wide_jobs_still_complete_retry_and_get_reaped` (an instance-wide
follow-up enqueued by a workspace transaction retries and completes, an
orphaned one is reaped and runs again, a fatal one is recorded failed).
The first fails against the previous policy.

### The maintenance scope (item 3 of the issue)

`campbooks.maintenance_scope = 'on'` (`kernel::db::MAINTENANCE_SCOPE_SETTING`,
crate-private), opened by `kernel::db::begin_maintenance(pool)` and
`JobContext::maintenance_tx()` (which refuses a workspace's job, like
`kernel_tx`). Only the maintenance policies trust it:

- `connectors.sync_due`: `maintenance_reads_schedule` (`FOR SELECT`) and
  `maintenance_leases_schedule` (`FOR UPDATE`).
- `conversations.needs_backfills`: `maintenance_reads_backfills`
  (`FOR SELECT`) and `maintenance_starts_backfills` (`FOR UPDATE`, only to
  `running`).

No kernel table admits it, so the dispatcher cannot reach the queue across
workspaces: `jobs::enqueue` refuses a workspace's job in a transaction that
does not work in that workspace. Each dispatcher sets the run's workspace
(`db::set_workspace`, public already) just before its enqueue, in the same
transaction, so the lease and the enqueue stay atomic as before. Setting a
workspace gives nothing `begin_for` does not already give any module code.
The needs dispatcher is an instance-wide job, so the worker still turns the
kernel scope on for its own completion (path 7) after it returns.

Considered: one transaction per workspace for the enqueues. It would split
the lease from the enqueue (a crash between them leaves streams leased for
15 minutes with no run) for no gain in reach.

`cargo xtask arch` treats the new names like the kernel scope's: the
setting's text, its constant and its last word are the kernel's; the
openers are allowed only under `[[access.maintenance_scope]]`, a separate
list, so an entry for one scope does not open the other. With both
dispatchers moved, `[[access.kernel_scope]]` is empty and no module table
admits the kernel scope (the server's `maintenance_policies_only_read_and_lease`
test pins both).

### Arch-rule gaps (item 2)

In `xtask/src/access.rs`:

- One small lexer (`lex`) now cuts Rust source into code, comments and
  literals; `without_comments` and the new `code_only` (comments and literals
  blanked, byte for byte) are built on it.
- **C raw strings.** `raw_string_at` accepts a `c` prefix the way it accepts
  `b`, so `cr#"x"//"#; begin_kernel(pool);` no longer hides the call.
- **The bare word.** Outside the kernel, the word `kernel_scope` (or
  `maintenance_scope`) on its own, in code or in a string, in any case and in
  tests too, is a violation unless the file is listed for that scope:
  `'campbooks.' || 'kernel_scope'`, `campbooks."kernel_scope"`,
  `campbooks.KERNEL_SCOPE`, `concat!("campbooks.", stringify!(kernel_scope))`,
  `format!("campbooks.{}", "kernel_scope")` and the raw identifier
  `r#kernel_scope` are all caught. Longer names (`kernel_scope_pool`,
  `kernel_scope_setting`) are not the word.
- **The test-module cut.** `without_tests` looks for
  `#[cfg(test)]\nmod tests {` in `code_only(text)`, so the same text in a
  string, a raw string or a comment no longer ends the check. Code after a
  real test module is clippy's `items_after_test_module`.
- **The BFF.** The `crates/server/src/bff/` entry became two file entries
  (`store.rs`, the session store; `mod.rs`, the token refresh's row lock);
  a test checks that no `pool_access` entry exempts a server directory.

Each has an xtask unit test.

### Left open (what a text check still cannot catch)

- **Names split below the word.** `concat!("campbooks.kernel", "_scope")`,
  `format!("campbooks.{}_scope", "kernel")`, `'kernel_' || 'scope'`, a name
  read from configuration or a file, or built from `char`s. Only the whole
  word is matched; following values needs evaluation the text check does not
  have.
- **Setting it from SQL kept elsewhere**: a `const` from another file, a
  stored function, a migration (migrations are reviewed SQL, and the
  migration check keeps each to its own schema).
- **Executors and SQL the regex does not name** (from #129): a pool held
  under another name, a generic `impl PgExecutor`, SQL built at runtime.
- **Macros** that expand to a call of an opener under a name the check does
  not see (a `macro_rules!` taking the identifier in pieces).
- **Lexer corner cases**: the lexer is not rustc's (a lifetime next to a
  quote, an unterminated literal); a file that confuses it is more likely to
  fail the check than pass it, but not always.

Row-level security is the backstop for all of them: a path that sets nothing
sees nothing, and the only setting module tables trust is the maintenance
scope's, on two tables, for ids, times and states only.

## Risks

- **Background work that silently stops.** A missed path that changes an
  instance-wide row from a workspace transaction would now change nothing
  rather than fail. Mitigation: the table above, the whole workspace's
  tests as the runtime role, and the new tests. For the dispatchers: the
  connectors kit runs `dispatch` as the runtime role (the sync tests fail if
  it stops leasing or enqueueing), and the needs backfill tests run the
  dispatcher job.
- **Deploy order.** The migrations and the code ship together. A process
  still running the previous build after the migrations ran would keep
  working for item 1 (it never changed instance-wide rows from a workspace
  transaction), but its two dispatchers (kernel scope) would see no rows of
  their index: streams stop being polled and needs backfills stop starting
  until it is replaced. The server migrates on start, so a single-instance
  restart has no such window.
- **Lock timeout.** Each migration takes `ACCESS EXCLUSIVE` on its tables
  for the DROP and CREATE; `lock_timeout = 5s` makes a busy queue fail the
  migration (and the start) instead of queueing every job statement behind
  it; the next start tries again.

## Rollback

sqlx has no down migrations. An image older than this change still works
with the kernel policy of item 1, but its dispatchers need the kernel-scope
maintenance policies, and sqlx refuses to start with applied migrations it
does not know. **Ops step: before redeploying an older tag, run this once as
the migration login.** It restores the exact policies of
`20261020200000_kernel__kernel_scope` (events, jobs),
`20261020200100_connectors__kernel_scope` and
`20261020200200_conversations__kernel_scope`, and forgets this change's
three migrations so a later upgrade applies them again. Checked against a
database migrated only to the base: identical `pg_policies` and
`_sqlx_migrations`, and re-upgrading applies cleanly.

```sql
-- Roll back campbooks-cloud#196 BEFORE redeploying an image older than it.
-- Run once, as the migration login (the tables' owner). Restores the exact
-- policies of 20261020200000 (kernel.events, kernel.jobs), 20261020200100
-- (connectors.sync_due) and 20261020200200 (conversations.needs_backfills),
-- and forgets this change's three migrations, so the older image starts
-- (sqlx refuses applied migrations it does not know) and a later upgrade
-- applies them again.
BEGIN;
SET LOCAL lock_timeout = '5s';
DO $$
DECLARE
    t text;
BEGIN
    FOREACH t IN ARRAY ARRAY['events', 'jobs']
    LOOP
        EXECUTE format('DROP POLICY instance_rows_read ON kernel.%I', t);
        EXECUTE format('DROP POLICY instance_rows_insert ON kernel.%I', t);
        EXECUTE format('DROP POLICY workspace_isolation ON kernel.%I', t);
        EXECUTE format(
            'CREATE POLICY workspace_isolation ON kernel.%I USING '
            '(current_setting(''campbooks.kernel_scope'', true) = ''on'' '
            ' OR (workspace_id IS NULL '
            '     AND nullif(current_setting(''campbooks.workspace_id'', true), '''') IS NOT NULL) '
            ' OR workspace_id = nullif(current_setting(''campbooks.workspace_id'', true), ''''))',
            t);
    END LOOP;
END
$$;
DROP POLICY maintenance_reads_schedule ON connectors.sync_due;
DROP POLICY maintenance_leases_schedule ON connectors.sync_due;
CREATE POLICY maintenance_reads_schedule ON connectors.sync_due
    FOR SELECT
    USING (current_setting('campbooks.kernel_scope', true) = 'on');
CREATE POLICY maintenance_leases_schedule ON connectors.sync_due
    FOR UPDATE
    USING (current_setting('campbooks.kernel_scope', true) = 'on')
    WITH CHECK (current_setting('campbooks.kernel_scope', true) = 'on');
DROP POLICY maintenance_reads_backfills ON conversations.needs_backfills;
DROP POLICY maintenance_starts_backfills ON conversations.needs_backfills;
CREATE POLICY maintenance_reads_backfills ON conversations.needs_backfills FOR SELECT
    USING (current_setting('campbooks.kernel_scope', true) = 'on');
CREATE POLICY maintenance_starts_backfills ON conversations.needs_backfills FOR UPDATE
    USING (current_setting('campbooks.kernel_scope', true) = 'on')
    WITH CHECK (current_setting('campbooks.kernel_scope', true) = 'on'
                AND state = 'running');
DELETE FROM _sqlx_migrations
 WHERE version IN (20261021010000, 20261021010100, 20261021010200);
COMMIT;
```

Every process must run the same side of the change: upgrade (or roll back)
the API and the worker together.
