-- Notion databases as tables (notion-databases, campbooks-cloud#377), the
-- connectors side. A linked Notion database (a data source, from API version
-- 2026-03-11) is one change stream like a linked page: the same `pages`
-- capability, with the data source's id as its `mailbox_key`, and the same
-- `notion_links` row, told apart by `kind`. The webhook finds either kind by
-- the Notion id (read only: ids) and only makes the stream due.
--
-- `notion_table_changes` are the schema, row and cell changes the pages
-- module asked for, recorded once it confirmed them, bound to the request,
-- the stream, its connection and its data source with a MAC under a key the
-- database never holds, and made by the stream's next run before it reads.
-- Ids, kinds, times, hashes and reason codes only: no column name, option or
-- value (they are in the blob the pages module wrote).
--
-- Written to run again safely: every object is created only when missing.
--
-- Wait at most 5 s for a lock: a busy instance fails this migration fast,
-- and the next start retries it, rather than stalling.
SET LOCAL lock_timeout = '5s';

ALTER TABLE connectors.notion_links
    ADD COLUMN IF NOT EXISTS kind text NOT NULL DEFAULT 'page'
    CHECK (kind IN ('page', 'data_source'));

CREATE TABLE IF NOT EXISTS connectors.notion_table_changes (
    -- UUIDv7: the order changes were recorded in, which is the order they
    -- are made in.
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    stream_id uuid NOT NULL REFERENCES connectors.streams (id) ON DELETE CASCADE,
    -- The pages module's request, the table it is for and the item's place.
    request_id uuid NOT NULL,
    table_id uuid NOT NULL,
    seq integer NOT NULL CHECK (seq >= 0),
    kind text NOT NULL CHECK (kind IN (
        'schema_add', 'schema_update', 'schema_remove', 'row_create', 'cell_update',
        'row_trash')),
    -- The column or row in the pages module, and the column a cell is in.
    subject_id uuid NOT NULL,
    property_id uuid,
    -- The column and the row at Notion.
    external_property text CHECK (external_property IS NULL OR char_length(external_property) <= 200),
    external_page uuid,
    -- What Notion held for the column or the cell at the last sync, and the
    -- whole schema's hash then (hexadecimal).
    base_hash text CHECK (base_hash IS NULL OR char_length(base_hash) = 64),
    schema_hash text CHECK (schema_hash IS NULL OR char_length(schema_hash) = 64),
    -- When Notion last edited the row at the last sync (row trash).
    base_edited_at timestamptz,
    -- The member confirmed removing the column at Notion too.
    confirmed boolean NOT NULL DEFAULT false,
    -- The blob reference of the request's document, stored and verified by
    -- the pages module: what is written is that document and no other.
    body jsonb NOT NULL,
    mac bytea,
    state text NOT NULL DEFAULT 'pending'
        CHECK (state IN ('pending', 'applied', 'skipped', 'failed')),
    -- Why it ended as it did (a stable code, never Notion's words).
    reason text,
    attempts integer NOT NULL DEFAULT 0,
    -- What Notion answered (applied): the column's or the row's id, the edit
    -- time, and the hash of what it holds now.
    result_external text CHECK (result_external IS NULL OR char_length(result_external) <= 200),
    result_edited_at timestamptz,
    result_hash text CHECK (result_hash IS NULL OR char_length(result_hash) = 64),
    created_at timestamptz NOT NULL DEFAULT now(),
    done_at timestamptz,
    UNIQUE (request_id, seq)
);
CREATE INDEX IF NOT EXISTS notion_table_changes_pending_idx
    ON connectors.notion_table_changes (stream_id, id) WHERE state = 'pending';
CREATE INDEX IF NOT EXISTS notion_table_changes_done_idx
    ON connectors.notion_table_changes (done_at) WHERE state <> 'pending';

ALTER TABLE connectors.notion_table_changes ENABLE ROW LEVEL SECURITY;
ALTER TABLE connectors.notion_table_changes FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS workspace_isolation ON connectors.notion_table_changes;
CREATE POLICY workspace_isolation ON connectors.notion_table_changes USING
    (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);

GRANT SELECT, INSERT, UPDATE, DELETE ON connectors.notion_table_changes TO campbooks_app;
