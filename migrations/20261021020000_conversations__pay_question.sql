-- Pay asks "Is it paid?" (campbooks-cloud#167).
--
-- Pay was raised whenever an accepted `asks_action` met mail sorted to
-- Receipts, so invoices already paid and payment confirmations became "Pay".
-- Now a `money` question (owed / paid / neither) raises Pay, only for a
-- payment still owed, as a question: a need's question may be `money`, and
-- a member answers it Already paid or Still to pay.
--
-- Then every inbox is queued for the needs backfill: each recent
-- conversation's need is worked out again (a Pay need the old rule raised
-- is withdrawn as `superseded`), and a trusted decide model is asked about
-- each conversation's newest recent inbound message that has no checked
-- answer to every triage question (so it answers `money`). Idempotent: a
-- second run queues the inboxes again, which changes no need that did not
-- change.

-- Wait at most 10 s for a lock: a busy instance fails this migration fast,
-- and the next start retries it, rather than stalling.
SET LOCAL lock_timeout = '10s';

-- Widened NOT VALID, so the swap holds the table's exclusive lock only for a
-- moment; validated by `…000100_conversations__pay_question_validate`.
ALTER TABLE conversations.needs DROP CONSTRAINT needs_question_check;
ALTER TABLE conversations.needs ADD CONSTRAINT needs_question_check
    CHECK (question IN ('needs_reply', 'asks_action', 'suspicious', 'money')) NOT VALID;
ALTER TABLE conversations.needs DROP CONSTRAINT needs_verdict_check;
ALTER TABLE conversations.needs ADD CONSTRAINT needs_verdict_check
    CHECK (verdict IN ('done', 'not_needed', 'confirmed', 'already_paid', 'still_to_pay'))
    NOT VALID;

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
