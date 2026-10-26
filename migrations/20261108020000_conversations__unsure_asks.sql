-- A guess is never shown as sure (campbooks-cloud#372).
--
-- A model's "needs a reply" or "asks for an action" raises a sure need only
-- from a bar measured on the eval set (`policy.rs`), and never when the
-- member is only copied; below it, from the check floor, it is a question.
-- The policy is worked out over the stored answers, so needs already open
-- under the old policy are given a clock due now: the minute dispatcher
-- (`conversations.dispatch_need_clocks`) has `conversations.wake_needs` work
-- each one out again, once. A sure Reply or Do that no longer qualifies
-- becomes a question, or is cleared as superseded when its answer is below
-- the check floor. No model is asked and no rule runs again. Idempotent: a
-- second run only wakes needs that already stand under this policy, which
-- changes nothing.
--
-- A member who answers such a question can say someone else has the move
-- ("Nudge Ana…"). The answer is kept with the verdict, so the conversation
-- waits on that person (`verdict_awaited_*`), and the second correction it
-- files (the `awaited` one) is withdrawn with it on Undo
-- (`verdict_extra_correction_id`).
--
-- `needs_backfills.moves_asked` says whether the backfill of an inbox ran
-- with whose move it is on (`CAMPBOOKS_CONVERSATIONS__MOVES__ENABLED`).
-- Backfills that finished without it run once more when it is switched on;
-- with it off nothing is reset.

-- Wait at most 10 s for a lock: a busy instance fails this migration fast,
-- and the next start retries it, rather than stalling.
SET LOCAL lock_timeout = '10s';

-- Catalog-only: nullable columns and a constant default.
ALTER TABLE conversations.needs
    ADD COLUMN verdict_awaited_name text,
    ADD COLUMN verdict_awaited_address text,
    ADD COLUMN verdict_extra_correction_id uuid;
ALTER TABLE conversations.needs_backfills
    ADD COLUMN moves_asked boolean NOT NULL DEFAULT false;

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
