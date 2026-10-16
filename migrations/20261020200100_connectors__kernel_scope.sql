SET LOCAL lock_timeout = '5s';

-- The one-minute dispatcher reads and leases the scheduling index across
-- workspaces. Its maintenance policies admitted any transaction that set no
-- workspace; they now admit only the kernel scope
-- (`campbooks.kernel_scope = 'on'`, set by `kernel::db::begin_kernel`, which
-- `architecture.toml` lets this module's scheduler call), so a transaction
-- that set nothing sees no row (campbooks-cloud#129). Still read (`FOR
-- SELECT`) and lease (`FOR UPDATE`) only: never insert or delete.

DROP POLICY maintenance_reads_schedule ON connectors.sync_due;
DROP POLICY maintenance_leases_schedule ON connectors.sync_due;

CREATE POLICY maintenance_reads_schedule ON connectors.sync_due
    FOR SELECT
    USING (current_setting('campbooks.kernel_scope', true) = 'on');

CREATE POLICY maintenance_leases_schedule ON connectors.sync_due
    FOR UPDATE
    USING (current_setting('campbooks.kernel_scope', true) = 'on')
    WITH CHECK (current_setting('campbooks.kernel_scope', true) = 'on');
