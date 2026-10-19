-- Who has the next move (campbooks-cloud#293): waiting on someone, and Nudge.
--
-- A conversation whose newest inbound message asks nothing of the member, but
-- in which the sender or someone else on the email has the next move, gets a
-- `waiting` need on that person: their name and address as the headers gave
-- them (`awaited_name`, `awaited_address`, held with the conversation like
-- its messages' addresses), when Scout nudges (`nudge_at`), and how many
-- times the member wrote to them since (`nudges`). At `nudge_at` the need
-- clock turns it into a `nudge` need; mail from that person reaching the
-- inbox clears it as `heard_back`. A Nudge is a draft of its own kind,
-- `nudge`. The question behind a wait is `next_move`.
--
-- Then every inbox is queued for the needs backfill: each conversation's
-- need is worked out again (recent mail with no open need may now wait on
-- someone), and, with the action catalogue on and a checked `decide` role,
-- recent inbound mail with no open need and no `next_move` answer is asked
-- the two move questions once, within the workspace's daily AI limit.
-- Idempotent: a second run queues the inboxes again, which changes no need
-- that did not change and asks no question already answered.

-- Wait at most 10 s for a lock: a busy instance fails this migration fast,
-- and the next start retries it, rather than stalling.
SET LOCAL lock_timeout = '10s';

-- Catalog-only: nullable columns, and a constant default.
ALTER TABLE conversations.needs
    ADD COLUMN awaited_name text,
    ADD COLUMN awaited_address text,
    ADD COLUMN nudge_at timestamptz,
    ADD COLUMN nudges integer NOT NULL DEFAULT 0;

-- Widened NOT VALID, so the swap holds the table's exclusive lock only for a
-- moment; validated by `…090100_conversations__needs_waiting_validate`. The
-- lists are the whole vocabulary, so they keep what earlier migrations added.
ALTER TABLE conversations.needs DROP CONSTRAINT needs_reason_check;
ALTER TABLE conversations.needs ADD CONSTRAINT needs_reason_check
    CHECK (reason IN ('reply', 'do', 'pay', 'review', 'worth_a_look', 'waiting', 'nudge'))
    NOT VALID;
ALTER TABLE conversations.needs DROP CONSTRAINT needs_question_check;
ALTER TABLE conversations.needs ADD CONSTRAINT needs_question_check
    CHECK (question IN ('needs_reply', 'asks_action', 'suspicious', 'money', 'worth_a_look',
                        'still_to_do', 'still_needs_reply', 'next_move')) NOT VALID;
ALTER TABLE conversations.needs DROP CONSTRAINT needs_cleared_because_check;
ALTER TABLE conversations.needs ADD CONSTRAINT needs_cleared_because_check
    CHECK (cleared_because IN
        ('replied', 'archived', 'trashed', 'done', 'not_needed', 'superseded', 'rule', 'opened',
         'lapsed', 'heard_back'))
    NOT VALID;
ALTER TABLE conversations.drafts DROP CONSTRAINT drafts_kind_check;
ALTER TABLE conversations.drafts ADD CONSTRAINT drafts_kind_check
    CHECK (kind IN ('new', 'reply', 'reply_all', 'forward', 'nudge')) NOT VALID;

-- Hearing back: the open waits on an address, when mail from it (or to it)
-- reaches one of the workspace's inboxes. `needs` holds no inbox: the
-- conversation says which, and only the waits on that address are read.
CREATE INDEX needs_awaited_idx
    ON conversations.needs (workspace_id, lower(awaited_address))
    WHERE state = 'open' AND reason IN ('waiting', 'nudge');

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
