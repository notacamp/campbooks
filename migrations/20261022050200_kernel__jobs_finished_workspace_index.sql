-- no-transaction
-- Finished jobs by workspace (campbooks-cloud#65). A workspace's purge and
-- the hourly sweep of purged workspaces delete a workspace's finished jobs
-- in batches (`workspace_id = $1 AND state IN ('succeeded', 'failed')`, then
-- `LIMIT`); without an index each batch scanned the queue.
--
-- Built CONCURRENTLY, so enqueues go on during the build, in a migration of
-- its own with a single statement (sqlx runs a file as one query, and a
-- concurrent build cannot run in a transaction). A build that fails or
-- whose process dies leaves an INVALID index of that name, which the server
-- drops before migrating (`migrate` in crates/server/src/main.rs); IF NOT
-- EXISTS keeps a VALID index built just before the process died, before
-- sqlx recorded the migration. See 20261021130000.
CREATE INDEX CONCURRENTLY IF NOT EXISTS jobs_finished_workspace_idx
    ON kernel.jobs (workspace_id)
    WHERE state IN ('succeeded', 'failed') AND workspace_id IS NOT NULL;
