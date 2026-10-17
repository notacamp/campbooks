SET LOCAL lock_timeout = '5s';

-- The one-minute dispatcher reads and leases the scheduling index across
-- workspaces. Its maintenance policies admitted the whole kernel scope
-- (20261020200100), which also reaches every workspace's events, jobs,
-- changes and blobs. They now admit only the maintenance scope
-- (`campbooks.maintenance_scope = 'on'`, set by
-- `kernel::db::begin_maintenance`, which `architecture.toml` lets this
-- module's scheduler call), which no kernel table admits: the dispatcher
-- enqueues each stream's run in that stream's workspace (campbooks-cloud#196).
-- Still read (`FOR SELECT`) and lease (`FOR UPDATE`) only: never insert or
-- delete.

DROP POLICY maintenance_reads_schedule ON connectors.sync_due;
DROP POLICY maintenance_leases_schedule ON connectors.sync_due;

CREATE POLICY maintenance_reads_schedule ON connectors.sync_due
    FOR SELECT
    USING (current_setting('campbooks.maintenance_scope', true) = 'on');

CREATE POLICY maintenance_leases_schedule ON connectors.sync_due
    FOR UPDATE
    USING (current_setting('campbooks.maintenance_scope', true) = 'on')
    WITH CHECK (current_setting('campbooks.maintenance_scope', true) = 'on');
