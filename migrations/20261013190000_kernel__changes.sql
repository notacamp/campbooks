-- Kernel infrastructure: the client change feed (ADR 0011). Domain events
-- that declare the records they changed leave one row per change here, in
-- the publisher's transaction, unsequenced. The `kernel.sequence_changes`
-- job then numbers a workspace's changes densely (`seq`) in commit order, so
-- a client resuming from cursor N never misses a change committed later
-- with a smaller number. Rows carry ids only, never record contents.
-- Written and read only through `campbooks_kernel::changes`.
CREATE TABLE kernel.changes (
    id uuid PRIMARY KEY DEFAULT kernel.uuid_v7(),
    -- Insertion order across all producers: the order the sequencer numbers
    -- a workspace's committed changes in (a transaction's changes keep the
    -- order it declared them in).
    published bigint GENERATED ALWAYS AS IDENTITY,
    workspace_id text NOT NULL,
    -- The cursor: NULL until sequenced, then dense per workspace from 1.
    seq bigint CHECK (seq > 0),
    -- The event in `kernel.events` that declared the change.
    event_id uuid NOT NULL,
    -- The changed record: `<module>.<record>` and its id.
    record_type text NOT NULL,
    record_id text NOT NULL,
    op text NOT NULL CHECK (op IN ('upsert', 'delete')),
    -- What the record lives in when not every member may see it (an inbox,
    -- a calendar, a folder); both NULL when every member may.
    container_type text,
    container_id text,
    occurred_at timestamptz NOT NULL DEFAULT now(),
    CHECK ((container_type IS NULL) = (container_id IS NULL))
);

-- Reading a workspace's feed from a cursor; one change per cursor.
CREATE UNIQUE INDEX changes_workspace_seq_idx ON kernel.changes (workspace_id, seq)
    WHERE seq IS NOT NULL;
-- The sequencer's work: a workspace's unnumbered changes in publish order.
CREATE INDEX changes_unsequenced_idx ON kernel.changes (workspace_id, published)
    WHERE seq IS NULL;
-- Pruning by age.
CREATE INDEX changes_occurred_at_idx ON kernel.changes (occurred_at);

-- The last cursor handed out per workspace.
CREATE TABLE kernel.change_heads (
    workspace_id text PRIMARY KEY,
    head bigint NOT NULL CHECK (head >= 0),
    updated_at timestamptz NOT NULL DEFAULT now()
);

-- Like the event log, these are infrastructure read and written only
-- through the kernel's API, which always filters by the workspace the caller
-- was admitted to; no row-level security policy.
GRANT SELECT, INSERT, UPDATE, DELETE ON kernel.changes, kernel.change_heads TO campbooks_app;
