SET LOCAL lock_timeout = '5s';

-- The needs backfill's dispatcher reads pending rows and starts them across
-- workspaces. Its maintenance policies admitted the whole kernel scope
-- (20261020200200), which also reaches every workspace's events, jobs,
-- changes and blobs. They now admit only the maintenance scope
-- (`campbooks.maintenance_scope = 'on'`, set by a job's `maintenance_tx`,
-- which `architecture.toml` lets this module's dispatcher call), which no
-- kernel table admits: the dispatcher enqueues each inbox's page job in that
-- inbox's workspace (campbooks-cloud#196). Still ids and states only, and a
-- row may only be started (`state = 'running'`).

DROP POLICY maintenance_reads_backfills ON conversations.needs_backfills;
DROP POLICY maintenance_starts_backfills ON conversations.needs_backfills;

CREATE POLICY maintenance_reads_backfills ON conversations.needs_backfills FOR SELECT
    USING (current_setting('campbooks.maintenance_scope', true) = 'on');
CREATE POLICY maintenance_starts_backfills ON conversations.needs_backfills FOR UPDATE
    USING (current_setting('campbooks.maintenance_scope', true) = 'on')
    WITH CHECK (current_setting('campbooks.maintenance_scope', true) = 'on'
                AND state = 'running');
