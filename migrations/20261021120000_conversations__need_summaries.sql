-- Scout's summary of the email behind a need (campbooks-cloud#204).
--
-- One row per message that raised a need, written once by the job
-- `conversations.summarize_need` and never regenerated when the item is
-- opened. The `complete` role writes the summary in every shipped locale in
-- one call, so a member reads it in their own language without any member
-- preference being known here; `texts` maps a locale tag to its text.
-- `failed` records a call that gave no usable answer, so a failing message is
-- not asked again for ever (one retry later, `attempts` counts them).
SET LOCAL lock_timeout = '10s';

CREATE TABLE conversations.need_summaries (
    message_id uuid PRIMARY KEY REFERENCES conversations.messages (id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL,
    state text NOT NULL CHECK (state IN ('written', 'failed')),
    texts jsonb NOT NULL DEFAULT '{}'
        CHECK (jsonb_typeof(texts) = 'object'),
    attempts smallint NOT NULL DEFAULT 1 CHECK (attempts > 0),
    written_at timestamptz NOT NULL DEFAULT now(),
    CHECK ((state = 'written') = (texts <> '{}'))
);

ALTER TABLE conversations.need_summaries ENABLE ROW LEVEL SECURITY;
ALTER TABLE conversations.need_summaries FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON conversations.need_summaries USING
    (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);

GRANT SELECT, INSERT, UPDATE, DELETE ON conversations.need_summaries TO campbooks_app;

-- Every inbox is queued for the needs backfill: its page job also queues a
-- summary for each open need that has none (no manual step on upgrade).
-- Idempotent: a second run queues the inboxes again and writes no summary
-- that exists.
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
