-- Home ordered by priority bands (campbooks-cloud#358): `need_changed` now
-- carries two facts about an open need, `warning` (it is the warning about
-- suspicious mail) and `from_person` (a person sent its message, from the
-- headers alone). Neither is stored here: both are worked out when the need
-- is announced.
--
-- `facts_version` says which facts the need was last announced with. A
-- refresh announces an open need again when it is older than the facts
-- `need_changed` carries now, even if the need itself did not change.
--
-- Then every open need is given a clock due now, so the minute dispatcher
-- (`conversations.dispatch_need_clocks`) has `conversations.wake_needs`
-- work each one out again and announce it with the new facts: no model is
-- asked and no rule runs again. Idempotent: a second run only wakes needs
-- that are already announced with these facts, which changes nothing.

-- Wait at most 10 s for a lock: a busy instance fails this migration fast,
-- and the next start retries it, rather than stalling.
SET LOCAL lock_timeout = '10s';

-- Catalog-only: a constant default.
ALTER TABLE conversations.needs ADD COLUMN facts_version smallint NOT NULL DEFAULT 0;

-- seed begin
-- Both tables force row-level security even for their owner, and no
-- workspace is set here: it is lifted for this statement only, inside the
-- migration's transaction.
ALTER TABLE conversations.needs NO FORCE ROW LEVEL SECURITY;
ALTER TABLE conversations.need_clocks NO FORCE ROW LEVEL SECURITY;
INSERT INTO conversations.need_clocks (conversation_id, workspace_id, wake_at)
SELECT conversation_id, workspace_id, now() FROM conversations.needs WHERE state = 'open'
ON CONFLICT (conversation_id) DO UPDATE
   SET wake_at = least(conversations.need_clocks.wake_at, EXCLUDED.wake_at);
ALTER TABLE conversations.need_clocks FORCE ROW LEVEL SECURITY;
ALTER TABLE conversations.needs FORCE ROW LEVEL SECURITY;
-- seed end
