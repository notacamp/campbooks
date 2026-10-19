-- Asks that lapse or go stale (campbooks-cloud#275).
--
-- The `complete` call that summarises a need's email also reads when what
-- it asks stops being possible (`lapses_at`) and by when the recipient must
-- act (`due_on`); both are kept with the message in `need_summaries`, with
-- when they were read (`dates_read_at`, NULL for summaries written before),
-- and the open need carries its own copy (`needs.lapses_at`; `due_on` is
-- already there). A need whose lapse time passed is cleared as `lapsed`; an
-- old undated ask becomes a question, `still_to_do` or `still_needs_reply`.
--
-- `need_clocks` says when each open need next changes on its own, so the
-- minute dispatcher (`conversations.dispatch_need_clocks`) knows which
-- workspaces to wake. It holds ids and a time; the dispatcher reads it in
-- the maintenance scope (read only) and enqueues each workspace's job.
--
-- Then every inbox is queued for the needs backfill: each open need is
-- settled against the clock and given its clock row, and each open need
-- whose summary was written before dates were read is read once more.
-- Idempotent: a second run queues the inboxes again, which changes no need
-- that did not change and asks for no date already read.

-- Wait at most 10 s for a lock: a busy instance fails this migration fast,
-- and the next start retries it, rather than stalling.
SET LOCAL lock_timeout = '10s';

-- Catalog-only: nullable columns.
ALTER TABLE conversations.needs ADD COLUMN lapses_at timestamptz;
ALTER TABLE conversations.need_summaries
    ADD COLUMN lapses_at timestamptz,
    ADD COLUMN due_on date,
    ADD COLUMN dates_read_at timestamptz;

-- Widened NOT VALID, so the swap holds the table's exclusive lock only for a
-- moment; validated by `…120100_conversations__needs_lapse_validate`.
ALTER TABLE conversations.needs DROP CONSTRAINT needs_question_check;
ALTER TABLE conversations.needs ADD CONSTRAINT needs_question_check
    CHECK (question IN ('needs_reply', 'asks_action', 'suspicious', 'money', 'still_to_do',
                        'still_needs_reply')) NOT VALID;
ALTER TABLE conversations.needs DROP CONSTRAINT needs_cleared_because_check;
ALTER TABLE conversations.needs ADD CONSTRAINT needs_cleared_because_check
    CHECK (cleared_because IN
        ('replied', 'archived', 'trashed', 'done', 'not_needed', 'superseded', 'rule', 'lapsed'))
    NOT VALID;

-- When an open need next changes with time alone: its lapse time, or when
-- its message leaves the needs window. One row per such need, gone with it.
CREATE TABLE conversations.need_clocks (
    conversation_id uuid PRIMARY KEY
        REFERENCES conversations.needs (conversation_id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL,
    wake_at timestamptz NOT NULL
);
-- The dispatcher's read (due rows across workspaces) and each workspace's
-- wake job (its due rows).
CREATE INDEX need_clocks_wake_idx ON conversations.need_clocks (wake_at);
CREATE INDEX need_clocks_workspace_idx ON conversations.need_clocks (workspace_id, wake_at);

ALTER TABLE conversations.need_clocks ENABLE ROW LEVEL SECURITY;
ALTER TABLE conversations.need_clocks FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON conversations.need_clocks USING
    (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);
-- The dispatcher reads which workspaces have a due row (ids and times only).
CREATE POLICY maintenance_reads_need_clocks ON conversations.need_clocks FOR SELECT
    USING (current_setting('campbooks.maintenance_scope', true) = 'on');

GRANT SELECT, INSERT, UPDATE, DELETE ON conversations.need_clocks TO campbooks_app;

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
