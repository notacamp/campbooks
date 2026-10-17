-- no-transaction
-- Job unique keys are per workspace (campbooks-cloud#209). The queue's old
-- unique index, `(kind, unique_key)` over unfinished jobs, spanned every
-- workspace: a workspace that knew or guessed another workspace's key
-- (most embed an id: `connectors.sync_stream:<stream>`) could hold it with a
-- row of its own that never runs, and the other workspace's enqueue then
-- found a duplicate (`ON CONFLICT DO NOTHING`) and was dropped.
--
-- The new index adds the workspace, with `NULLS NOT DISTINCT` (Postgres 15+;
-- Campbooks needs 17): the instance-wide rows (no workspace: schedule
-- ticks, account events' deliveries) still dedupe among themselves, and a
-- workspace's rows only among that workspace's. `kind, unique_key` lead, so
-- lookups by kind and key (a schedule's heal, the admin's retry) still use
-- it. The old index implies this one, so the build cannot find duplicates.
--
-- Built CONCURRENTLY, so enqueues go on during the build. That cannot run
-- in a transaction, and sqlx runs a migration file as one query, so the
-- build has a migration of its own with a single statement. A build that
-- fails or whose process dies leaves an INVALID index of that name: before
-- migrating, the server drops invalid indexes (`migrate` in
-- crates/server/src/main.rs, under the migration lock). IF NOT EXISTS keeps
-- a VALID index built just before the process died, before sqlx recorded
-- the migration. 20261021130200 drops the old index once this one exists.
CREATE UNIQUE INDEX CONCURRENTLY IF NOT EXISTS jobs_unique_key_per_workspace_idx
    ON kernel.jobs (kind, unique_key, workspace_id) NULLS NOT DISTINCT
    WHERE unique_key IS NOT NULL AND state IN ('pending', 'running');
