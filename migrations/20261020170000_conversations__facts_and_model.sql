-- Facts only say no; a model reads content (ADR 0018, campbooks-cloud#144).
--
-- The rules that read an email's text are gone. Their answers stay as a
-- record (never deleted), but stop counting: every rules answer given about
-- an envelope older than version 3 (triage context 2, the facts) is marked
-- not acted. Then every inbox is queued for the needs backfill, which works
-- out each recent conversation's need again, so a need only those answers
-- raised is withdrawn (`superseded`), and asks a trusted decide model about
-- each conversation's newest recent inbound message that has no checked
-- model answer. Idempotent: a second run marks nothing new and queues the
-- inboxes again, which changes no need that did not change.
--
-- Wait at most 10 s for a lock: a busy instance fails this migration fast,
-- and the next start retries it, rather than stalling.
SET LOCAL lock_timeout = '10s';

-- Both tables force row-level security even for their owner, and no
-- workspace is set here: it is lifted for these statements only, inside the
-- migration's transaction.
ALTER TABLE conversations.decisions NO FORCE ROW LEVEL SECURITY;
UPDATE conversations.decisions
   SET acted = false
 WHERE source = 'rules' AND state_version < 3 AND acted;
ALTER TABLE conversations.decisions FORCE ROW LEVEL SECURITY;

-- seed begin
ALTER TABLE conversations.inboxes NO FORCE ROW LEVEL SECURITY;
ALTER TABLE conversations.needs_backfills NO FORCE ROW LEVEL SECURITY;
INSERT INTO conversations.needs_backfills (inbox_id, workspace_id)
SELECT id, workspace_id FROM conversations.inboxes
ON CONFLICT (inbox_id) DO UPDATE
   SET state = 'pending', after_id = NULL, refreshed = 0, finished_at = NULL;
ALTER TABLE conversations.needs_backfills FORCE ROW LEVEL SECURITY;
ALTER TABLE conversations.inboxes FORCE ROW LEVEL SECURITY;
-- seed end
