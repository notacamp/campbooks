SET LOCAL lock_timeout = '5s';

-- Tombstones of purged workspaces (campbooks-cloud#65).
--
-- When `workspaces.purge_workspace` deletes a workspace's row it records the
-- workspace's id here, in the same transaction. The kernel's hourly sweep
-- (`kernel.sweep_purged_workspaces`) then deletes whatever of that
-- workspace still reaches the job queue, the event log and the change feed
-- afterwards (a module's purge that ran later and published, its own
-- finished job), whatever its age, except a delivery still pending or
-- running. A tombstone is dropped once nothing of its
-- workspace is left and it is older than 30 days.
--
-- It holds the id and the time only: no name, no member, nothing a person
-- wrote. It lives in the same database, so restoring a backup restores the
-- tombstones of that moment too; re-deleting the workspaces purged since is
-- the operator's step (`campbooks-server workspaces purge-again`, see
-- deploy/README.md).
CREATE TABLE kernel.purged_workspaces (
    workspace_id text PRIMARY KEY,
    purged_at timestamptz NOT NULL DEFAULT now()
);

-- The same isolation as every workspace-keyed kernel table: a workspace's
-- transaction (the purge job's own) reaches its own tombstone only, the
-- kernel scope (the sweep) reaches all of them, and a transaction that set
-- neither sees none.
ALTER TABLE kernel.purged_workspaces ENABLE ROW LEVEL SECURITY;
ALTER TABLE kernel.purged_workspaces FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON kernel.purged_workspaces USING
    (current_setting('campbooks.kernel_scope', true) = 'on'
     OR workspace_id = nullif(current_setting('campbooks.workspace_id', true), ''));

GRANT SELECT, INSERT, UPDATE, DELETE ON kernel.purged_workspaces TO campbooks_app;
