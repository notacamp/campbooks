-- An email you are only copied on reads as FYI (triage context 3).
--
-- The decide model used to see To and Cc as one `To:` line, so mail the
-- inbox's owner was only copied on could raise a "needs reply". Context 3
-- tells the model who was copied. Mail already decided under context 2 where
-- an inbox address is in Cc and none is in To is decided again: its acted
-- "needs reply" model answers stop counting now (the need is withdrawn by the
-- backfill below), and their `checked:` stamp is prefixed `superseded:`, so
-- no checked model answer remains and the needs backfill queues the model
-- pass (`conversations.decide_backlog`) for recent ones. Nothing is deleted,
-- and only mail copied to the inbox is asked again (not all recent mail).
-- Idempotent: a second run finds no acted, still-checked answer to mark and
-- queues the inboxes again, which changes no need that did not change.
--
-- Wait at most 10 s for a lock: a busy instance fails this migration fast,
-- and the next start retries it, rather than stalling.
SET LOCAL lock_timeout = '10s';

-- These tables force row-level security even for their owner, and no
-- workspace is set here: it is lifted for these statements only, inside the
-- migration's transaction.
ALTER TABLE conversations.decisions NO FORCE ROW LEVEL SECURITY;
ALTER TABLE conversations.messages NO FORCE ROW LEVEL SECURITY;
ALTER TABLE conversations.inboxes NO FORCE ROW LEVEL SECURITY;
UPDATE conversations.decisions d
   SET acted = false,
       model_version = 'superseded:' || d.model_version
  FROM conversations.messages m
  JOIN conversations.inboxes i ON i.id = m.inbox_id
 WHERE d.message_id = m.id
   AND d.source = 'model'
   AND d.question = 'needs_reply'
   AND d.acted
   AND d.state_version < 4
   AND d.model_version LIKE 'checked:%'
   AND m.direction = 'inbound'
   AND EXISTS (SELECT 1 FROM jsonb_array_elements(coalesce(m.recipients -> 'cc', '[]'::jsonb)) c
                WHERE lower(trim(c ->> 'address')) = lower(i.address))
   AND NOT EXISTS (SELECT 1 FROM jsonb_array_elements(coalesce(m.recipients -> 'to', '[]'::jsonb)) t
                    WHERE lower(trim(t ->> 'address')) = lower(i.address));
ALTER TABLE conversations.inboxes FORCE ROW LEVEL SECURITY;
ALTER TABLE conversations.messages FORCE ROW LEVEL SECURITY;
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
