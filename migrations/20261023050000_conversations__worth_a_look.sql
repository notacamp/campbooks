-- "Worth a look" (campbooks-cloud#274): a rule the member taught can keep a
-- sender's new mail in front of them until they have opened it once. It is a
-- need of its own (reason and question `worth_a_look`) that clears itself as
-- `opened` once the message is read.

-- Wait at most 10 s for a lock: a busy instance fails this migration fast,
-- and the next start retries it, rather than stalling.
SET LOCAL lock_timeout = '10s';

-- Widened NOT VALID, so the swap holds the table's exclusive lock only for a
-- moment; validated by `…050100_conversations__worth_a_look_validate`.
ALTER TABLE conversations.needs DROP CONSTRAINT needs_reason_check;
ALTER TABLE conversations.needs ADD CONSTRAINT needs_reason_check
    CHECK (reason IN ('reply', 'do', 'pay', 'review', 'worth_a_look')) NOT VALID;
ALTER TABLE conversations.needs DROP CONSTRAINT needs_question_check;
ALTER TABLE conversations.needs ADD CONSTRAINT needs_question_check
    CHECK (question IN ('needs_reply', 'asks_action', 'suspicious', 'money', 'worth_a_look'))
    NOT VALID;
ALTER TABLE conversations.needs DROP CONSTRAINT needs_cleared_because_check;
ALTER TABLE conversations.needs ADD CONSTRAINT needs_cleared_because_check
    CHECK (cleared_because IN
        ('replied', 'archived', 'trashed', 'done', 'not_needed', 'superseded', 'rule', 'opened'))
    NOT VALID;
