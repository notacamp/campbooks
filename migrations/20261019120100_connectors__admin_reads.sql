-- Instance administration (ADR 0015, slice S2): an administrator's
-- transaction (`kernel::db::begin_admin`, which sets `campbooks.admin_reads`)
-- may read every workspace's connections and streams: provider, address,
-- capabilities, health and sync state. Reads only (`FOR SELECT`). Never the
-- credentials, the Gmail index or anything else of a mailbox's content.
CREATE POLICY admin_reads ON connectors.connections FOR SELECT
    USING (current_setting('campbooks.admin_reads', true) = 'on');

CREATE POLICY admin_reads ON connectors.streams FOR SELECT
    USING (current_setting('campbooks.admin_reads', true) = 'on');
