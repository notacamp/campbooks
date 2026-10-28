-- Notion databases as tables (notion-databases, campbooks-cloud#377), the
-- pages side. A table can be linked to a database in a member's Notion
-- account and kept in step both ways, columns and values. The connection and
-- the data source are ids of the connectors module and of Notion: no
-- cross-schema key. Rows map through `external_links` (a row page and its
-- Notion page), so a row's body syncs like a synced page.
--
-- - `external_tables`: the link, one per table. A data source is linked to
--   one table per connection at a time (a stopped link frees it).
-- - `external_properties`: the three-way base of a column: the Notion
--   column's id and kind, and the name, type and options (hashes of the last
--   two) the last sync agreed, with a hash of what Notion held then.
-- - `external_cells`: the three-way base of a cell: the hash of the value
--   agreed at the last sync, and the hash of what Notion held then.
-- - `table_conflicts`: a column or a cell changed on both sides between two
--   syncs, with both versions, until the member chooses (and after, so the
--   choice can be undone). This is table content, kept with the table.
-- - `notion_table_requests`: the changes a member's edits asked for, bound to
--   the table, link and request with a MAC under a key the database never
--   holds, and waiting for the connector. Their words and values are in a
--   blob.
--
-- No value, column name or option name ever leaves these tables in an event
-- or a log.
--
-- Written to run again safely: every object is created only when missing.
--
-- Wait at most 5 s for a lock: a busy instance fails this migration fast,
-- and the next start retries it, rather than stalling.
SET LOCAL lock_timeout = '5s';

CREATE TABLE IF NOT EXISTS pages.external_tables (
    table_id uuid PRIMARY KEY REFERENCES pages.pages (id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL,
    connection_id uuid NOT NULL,
    notion_data_source_id uuid NOT NULL,
    state text NOT NULL CHECK (state IN ('syncing', 'paused', 'stopped')),
    stopped_reason text CHECK (stopped_reason IN (
        'stopped_by_member', 'disconnected', 'trashed_here', 'trashed_in_notion',
        'not_shared', 'member_left', 'notion_unavailable')),
    created_by uuid NOT NULL,
    -- How many conflicts wait for the member.
    conflicts integer NOT NULL DEFAULT 0 CHECK (conflicts >= 0),
    last_inbound_at timestamptz,
    last_outbound_at timestamptz,
    -- The time of the newest snapshot applied: an older one never is.
    last_read_at timestamptz,
    -- The hash of Notion's whole schema as of the last snapshot applied.
    schema_hash text CHECK (schema_hash IS NULL OR char_length(schema_hash) = 64),
    -- The title Notion had when it was last read.
    last_title text CHECK (last_title IS NULL OR char_length(last_title) <= 200),
    created_at timestamptz NOT NULL DEFAULT now(),
    stopped_at timestamptz,
    CHECK ((state = 'stopped') = (stopped_reason IS NOT NULL))
);
CREATE UNIQUE INDEX IF NOT EXISTS external_tables_notion_idx
    ON pages.external_tables (connection_id, notion_data_source_id) WHERE state <> 'stopped';
CREATE INDEX IF NOT EXISTS external_tables_connection_idx
    ON pages.external_tables (workspace_id, connection_id) WHERE state <> 'stopped';
CREATE INDEX IF NOT EXISTS external_tables_member_idx
    ON pages.external_tables (workspace_id, created_by) WHERE state <> 'stopped';

CREATE TABLE IF NOT EXISTS pages.external_properties (
    property_id uuid PRIMARY KEY REFERENCES pages.table_properties (id) ON DELETE CASCADE,
    table_id uuid NOT NULL REFERENCES pages.pages (id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL,
    -- The column at Notion, and the kind it has there.
    notion_property_id text NOT NULL CHECK (char_length(notion_property_id) BETWEEN 1 AND 200),
    notion_type text NOT NULL CHECK (char_length(notion_type) <= 40),
    -- What the last sync agreed: the name, and hashes of the type and of the
    -- options (so a change on this side is told from none).
    base_name text NOT NULL CHECK (char_length(base_name) <= 100),
    base_type_hash bytea NOT NULL CHECK (octet_length(base_type_hash) = 32),
    base_options_hash bytea NOT NULL CHECK (octet_length(base_options_hash) = 32),
    -- A hash of the column as Notion held it then.
    base_remote_hash bytea NOT NULL CHECK (octet_length(base_remote_hash) = 32),
    -- The member removed the column here and confirmed removing it at Notion
    -- too: only then does the sync remove it there.
    remove_confirmed boolean NOT NULL DEFAULT false,
    UNIQUE (table_id, notion_property_id)
);

CREATE TABLE IF NOT EXISTS pages.external_cells (
    row_id uuid NOT NULL REFERENCES pages.pages (id) ON DELETE CASCADE,
    property_id uuid NOT NULL REFERENCES pages.table_properties (id) ON DELETE CASCADE,
    table_id uuid NOT NULL,
    workspace_id uuid NOT NULL,
    -- The hash of the value agreed at the last sync (this side's form), and
    -- the hash of what Notion held then (the connector's form).
    base_hash bytea NOT NULL CHECK (octet_length(base_hash) = 32),
    base_remote_hash bytea NOT NULL CHECK (octet_length(base_remote_hash) = 32),
    base_edited_at timestamptz NOT NULL,
    PRIMARY KEY (row_id, property_id)
);
CREATE INDEX IF NOT EXISTS external_cells_table_idx ON pages.external_cells (table_id);

CREATE TABLE IF NOT EXISTS pages.table_conflicts (
    id uuid PRIMARY KEY,
    table_id uuid NOT NULL REFERENCES pages.pages (id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL,
    kind text NOT NULL CHECK (kind IN ('column', 'cell')),
    -- The column (and, for a cell, the row) in conflict.
    property_id uuid NOT NULL REFERENCES pages.table_properties (id) ON DELETE CASCADE,
    row_id uuid REFERENCES pages.pages (id) ON DELETE CASCADE,
    CHECK ((kind = 'cell') = (row_id IS NOT NULL)),
    -- Both versions as the conflict shows them (a column: name, type, config;
    -- a cell: the value in this side's form), and when each was made.
    ours jsonb NOT NULL,
    theirs jsonb NOT NULL,
    ours_at timestamptz NOT NULL,
    theirs_at timestamptz NOT NULL,
    -- What Notion held when it was found, to move the base to on a choice.
    theirs_remote_hash bytea NOT NULL CHECK (octet_length(theirs_remote_hash) = 32),
    raised_at timestamptz NOT NULL DEFAULT now(),
    -- The choice, once made: 'ours' or 'theirs', by whom and when; the base
    -- before it moved, so the choice can be undone.
    choice text CHECK (choice IS NULL OR choice IN ('ours', 'theirs')),
    resolved_by uuid,
    resolved_at timestamptz,
    undone_base jsonb
);
CREATE INDEX IF NOT EXISTS table_conflicts_open_idx
    ON pages.table_conflicts (table_id) WHERE choice IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS table_conflicts_one_open_cell_idx
    ON pages.table_conflicts (row_id, property_id) WHERE choice IS NULL AND kind = 'cell';
CREATE UNIQUE INDEX IF NOT EXISTS table_conflicts_one_open_column_idx
    ON pages.table_conflicts (property_id) WHERE choice IS NULL AND kind = 'column';

CREATE TABLE IF NOT EXISTS pages.notion_table_requests (
    id uuid PRIMARY KEY,
    table_id uuid NOT NULL REFERENCES pages.pages (id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL,
    connection_id uuid NOT NULL,
    notion_data_source_id uuid NOT NULL,
    -- The changes, in order: ids, kinds, hashes and times (no names or values).
    items jsonb NOT NULL,
    -- The blob reference of the document with what to write.
    body jsonb NOT NULL,
    mac bytea,
    state text NOT NULL DEFAULT 'waiting' CHECK (state IN ('waiting', 'done')),
    created_by uuid NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    done_at timestamptz
);
CREATE INDEX IF NOT EXISTS notion_table_requests_waiting_idx
    ON pages.notion_table_requests (table_id) WHERE state = 'waiting';

DO $$
DECLARE
    t text;
BEGIN
    FOREACH t IN ARRAY ARRAY['external_tables', 'external_properties', 'external_cells',
                             'table_conflicts', 'notion_table_requests']
    LOOP
        EXECUTE format('ALTER TABLE pages.%I ENABLE ROW LEVEL SECURITY', t);
        EXECUTE format('ALTER TABLE pages.%I FORCE ROW LEVEL SECURITY', t);
        EXECUTE format('DROP POLICY IF EXISTS workspace_isolation ON pages.%I', t);
        EXECUTE format(
            'CREATE POLICY workspace_isolation ON pages.%I USING '
            '(workspace_id = nullif(current_setting(''campbooks.workspace_id'', true), '''')::uuid)',
            t);
        EXECUTE format('GRANT SELECT, INSERT, UPDATE, DELETE ON pages.%I TO campbooks_app', t);
    END LOOP;
END
$$;
