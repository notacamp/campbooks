-- Scout's read of a message (campbooks-cloud#42).
--
-- `conversations.need_summaries` becomes `conversations.message_reads`: the
-- one `complete` call that reads an email now returns, besides the summary
-- of a need's email and its dates, the **items** (things that happen at a
-- time) and **asks** (obligations of the reader) found in it, and the
-- sender's own machine-readable dates are kept beside it as a second read of
-- the same message (layer `structured`, no model). Every row is kept: a
-- summary written before stays a read of layer `model`, with no items and
-- asks, and `read_at` NULL says they were never read.
--
-- `state` says where a read is: `queued` (asked for, not made yet), `read`
-- (was `written`) or `failed` (one call gave no usable answer, tried once
-- more later). A read of a message that no need is open on holds no summary,
-- so the old rule that `read` means a non-empty `texts` goes.
--
-- Then every inbox is queued for the needs backfill: each open need whose
-- message is flagged by the decide pass and has no read, one failed call, or
-- a read from before items and asks, is queued for the read. Idempotent: a
-- second run queues the inboxes again and asks for nothing that has a read.

-- Wait at most 10 s for a lock: a busy instance fails this migration fast,
-- and the next start retries it, rather than stalling.
SET LOCAL lock_timeout = '10s';

ALTER TABLE conversations.need_summaries RENAME TO message_reads;

-- Catalog-only: constant defaults and nullable columns.
ALTER TABLE conversations.message_reads
    ADD COLUMN layer text NOT NULL DEFAULT 'model',
    ADD COLUMN items jsonb NOT NULL DEFAULT '[]',
    ADD COLUMN asks jsonb NOT NULL DEFAULT '[]',
    ADD COLUMN triggered_by text,
    ADD COLUMN read_at timestamptz;

-- A message has one read per layer.
ALTER TABLE conversations.message_reads DROP CONSTRAINT need_summaries_pkey;
ALTER TABLE conversations.message_reads ADD CONSTRAINT message_reads_pkey
    PRIMARY KEY (message_id, layer);

-- The checks of the old table, by whatever name they were given, are
-- replaced by the vocabulary below.
DO $$
DECLARE
    old record;
BEGIN
    FOR old IN
        SELECT conname FROM pg_constraint
         WHERE conrelid = 'conversations.message_reads'::regclass AND contype = 'c'
    LOOP
        EXECUTE format('ALTER TABLE conversations.message_reads DROP CONSTRAINT %I', old.conname);
    END LOOP;
END
$$;

UPDATE conversations.message_reads SET state = 'read' WHERE state = 'written';

-- Added NOT VALID, so they hold the table's exclusive lock only for a
-- moment; validated by `…000200_conversations__message_reads_validate`.
ALTER TABLE conversations.message_reads
    ADD CONSTRAINT message_reads_state_check
        CHECK (state IN ('queued', 'read', 'failed')) NOT VALID,
    ADD CONSTRAINT message_reads_layer_check
        CHECK (layer IN ('structured', 'model')) NOT VALID,
    ADD CONSTRAINT message_reads_triggered_by_check
        CHECK (triggered_by IN ('need', 'open')) NOT VALID,
    ADD CONSTRAINT message_reads_texts_check
        CHECK (jsonb_typeof(texts) = 'object') NOT VALID,
    ADD CONSTRAINT message_reads_items_check
        CHECK (jsonb_typeof(items) = 'array') NOT VALID,
    ADD CONSTRAINT message_reads_asks_check
        CHECK (jsonb_typeof(asks) = 'array') NOT VALID,
    ADD CONSTRAINT message_reads_attempts_check CHECK (attempts >= 0) NOT VALID;

-- seed begin
-- Both tables force row-level security even for their owner, and no
-- workspace is set here: it is lifted for this statement only, inside the
-- migration's transaction.
ALTER TABLE conversations.inboxes NO FORCE ROW LEVEL SECURITY;
ALTER TABLE conversations.needs_backfills NO FORCE ROW LEVEL SECURITY;
INSERT INTO conversations.needs_backfills (inbox_id, workspace_id)
SELECT id, workspace_id FROM conversations.inboxes
ON CONFLICT (inbox_id) DO UPDATE
   SET state = 'pending', after_id = NULL, refreshed = 0, finished_at = NULL;
ALTER TABLE conversations.needs_backfills FORCE ROW LEVEL SECURITY;
ALTER TABLE conversations.inboxes FORCE ROW LEVEL SECURITY;
-- seed end
