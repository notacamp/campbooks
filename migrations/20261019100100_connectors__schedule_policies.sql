-- The one-minute dispatcher works across workspaces in a transaction that
-- sets none. Until now one policy (`maintenance_reads_schedule`) let such a
-- transaction do anything to the scheduling index; it now only reads it
-- (`FOR SELECT`) and leases due streams (`FOR UPDATE`, which `SELECT … FOR
-- UPDATE` also needs), and can no longer insert or delete rows.
DROP POLICY maintenance_reads_schedule ON connectors.sync_due;

CREATE POLICY maintenance_reads_schedule ON connectors.sync_due
    FOR SELECT
    USING (nullif(current_setting('campbooks.workspace_id', true), '') IS NULL);

CREATE POLICY maintenance_leases_schedule ON connectors.sync_due
    FOR UPDATE
    USING (nullif(current_setting('campbooks.workspace_id', true), '') IS NULL)
    WITH CHECK (nullif(current_setting('campbooks.workspace_id', true), '') IS NULL);
