SET LOCAL lock_timeout = '5s';

-- The needs backfill's dispatcher reads pending rows and starts them across
-- workspaces. Its maintenance policies admitted any transaction that set no
-- workspace; they now admit only the kernel scope
-- (`campbooks.kernel_scope = 'on'`, set by `kernel::db::begin_kernel`, which
-- `architecture.toml` lets this module's dispatcher call), so a transaction
-- that set nothing sees no row (campbooks-cloud#129). Still ids and states
-- only, and a row may only be started (`state = 'running'`).

DROP POLICY maintenance_reads_backfills ON conversations.needs_backfills;
DROP POLICY maintenance_starts_backfills ON conversations.needs_backfills;

CREATE POLICY maintenance_reads_backfills ON conversations.needs_backfills FOR SELECT
    USING (current_setting('campbooks.kernel_scope', true) = 'on');
CREATE POLICY maintenance_starts_backfills ON conversations.needs_backfills FOR UPDATE
    USING (current_setting('campbooks.kernel_scope', true) = 'on')
    WITH CHECK (current_setting('campbooks.kernel_scope', true) = 'on'
                AND state = 'running');
