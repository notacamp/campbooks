-- Tables in pages (pages-tables, campbooks-cloud#365, ADR 0022): a table is a
-- page of kind `table` whose rows are pages of kind `row` under it. This
-- adds the kind, the table's typed properties, one row per cell, and the
-- table's views. Access is the page chain's: nothing here stores who may
-- read what.
--
-- No value, name or title ever leaves these tables in an event or a log.
--
-- Written to run again safely: every object is created only when missing.
--
-- Wait at most 5 s for a lock: a busy instance fails this migration fast,
-- and the next start retries it, rather than stalling.
SET LOCAL lock_timeout = '5s';

ALTER TABLE pages.pages
    ADD COLUMN IF NOT EXISTS kind text NOT NULL DEFAULT 'page'
    CHECK (kind IN ('page', 'table', 'row'));

-- A table's counters: `revision` grows with every change of its schema,
-- views or rows (carried by `pages.table_changed`), and `next_number`
-- hands out the numbers of its `unique_id` property.
CREATE TABLE IF NOT EXISTS pages.tables (
    table_id uuid PRIMARY KEY REFERENCES pages.pages (id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL,
    revision bigint NOT NULL DEFAULT 1,
    next_number bigint NOT NULL DEFAULT 1
);

-- The columns of a table. `position` is a fractional-indexing key. A
-- removed property is kept with its values for 30 days so the removal can
-- be undone; a type change keeps the old type, config and values the same
-- way (`previous_*`, and `pages.row_values.previous`).
CREATE TABLE IF NOT EXISTS pages.table_properties (
    id uuid PRIMARY KEY,
    table_id uuid NOT NULL REFERENCES pages.pages (id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL,
    name text NOT NULL CHECK (char_length(name) BETWEEN 1 AND 100),
    type text NOT NULL CHECK (type IN (
        'title', 'text', 'number', 'select', 'multi_select', 'status', 'date',
        'checkbox', 'person', 'files', 'url', 'email', 'phone', 'relation',
        'campbooks_link', 'created', 'edited', 'unique_id',
        'notion_formula', 'notion_rollup', 'notion_people', 'external')),
    config jsonb NOT NULL DEFAULT '{}',
    position text NOT NULL CHECK (position <> '' AND char_length(position) <= 64),
    created_by uuid NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    removed_at timestamptz,
    previous_type text,
    previous_config jsonb,
    converted_at timestamptz
);
CREATE INDEX IF NOT EXISTS table_properties_table_idx
    ON pages.table_properties (table_id, position);
-- Exactly one title property per table.
CREATE UNIQUE INDEX IF NOT EXISTS table_properties_one_title_idx
    ON pages.table_properties (table_id) WHERE type = 'title';
CREATE INDEX IF NOT EXISTS table_properties_removed_idx
    ON pages.table_properties (removed_at) WHERE removed_at IS NOT NULL;

-- The rows of a table: their number (the `unique_id` property) and the
-- table they belong to. The row itself is the page.
CREATE TABLE IF NOT EXISTS pages.table_rows (
    row_id uuid PRIMARY KEY REFERENCES pages.pages (id) ON DELETE CASCADE,
    table_id uuid NOT NULL REFERENCES pages.pages (id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL,
    number bigint NOT NULL,
    UNIQUE (table_id, number)
);

-- One row per cell. The value is validated against its property's type
-- before it is stored, and every cell has a version of its own, so two
-- people editing two cells of a row never collide. The title, created,
-- edited and ID properties are not stored here: they come from the page.
CREATE TABLE IF NOT EXISTS pages.row_values (
    row_id uuid NOT NULL REFERENCES pages.pages (id) ON DELETE CASCADE,
    property_id uuid NOT NULL REFERENCES pages.table_properties (id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL,
    table_id uuid NOT NULL,
    value jsonb,
    version bigint NOT NULL DEFAULT 1,
    -- What the cell held before its property changed type; cleared by the
    -- next edit of the cell.
    previous jsonb,
    updated_by uuid NOT NULL,
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (row_id, property_id),
    CHECK (pg_column_size(value) <= 16384)
);
CREATE INDEX IF NOT EXISTS row_values_property_idx ON pages.row_values (property_id);
CREATE INDEX IF NOT EXISTS row_values_table_idx ON pages.row_values (table_id);

-- The views of a table: how it is filtered, sorted and shown.
CREATE TABLE IF NOT EXISTS pages.table_views (
    id uuid PRIMARY KEY,
    table_id uuid NOT NULL REFERENCES pages.pages (id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL,
    name text NOT NULL CHECK (char_length(name) BETWEEN 1 AND 100),
    type text NOT NULL CHECK (type IN ('table', 'board', 'list', 'calendar', 'gallery')),
    position text NOT NULL CHECK (position <> '' AND char_length(position) <= 64),
    config jsonb NOT NULL DEFAULT '{}',
    created_by uuid NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS table_views_table_idx ON pages.table_views (table_id, position);

DO $$
DECLARE
    t text;
BEGIN
    FOREACH t IN ARRAY ARRAY['tables', 'table_properties', 'table_rows', 'row_values',
                             'table_views']
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

-- The daily purge also finds, across workspaces, which hold a property
-- removed, or changed of type, more than 30 days ago: read only, and it reads nothing but ids and
-- times.
DROP POLICY IF EXISTS maintenance_reads_removed_properties ON pages.table_properties;
CREATE POLICY maintenance_reads_removed_properties ON pages.table_properties FOR SELECT
    USING (current_setting('campbooks.maintenance_scope', true) = 'on' AND (removed_at IS NOT NULL OR converted_at IS NOT NULL));
