-- Validate the checks `…090000_conversations__needs_waiting` widened NOT
-- VALID (campbooks-cloud#293). A migration of its own, so it runs in a
-- transaction of its own: VALIDATE CONSTRAINT takes only SHARE UPDATE
-- EXCLUSIVE, so reads and writes go on while it scans the rows.

-- Never wait long for a lock on a busy table: fail and retry the deploy.
SET LOCAL lock_timeout = '5s';

ALTER TABLE conversations.needs VALIDATE CONSTRAINT needs_reason_check;
ALTER TABLE conversations.needs VALIDATE CONSTRAINT needs_question_check;
ALTER TABLE conversations.needs VALIDATE CONSTRAINT needs_cleared_because_check;
ALTER TABLE conversations.drafts VALIDATE CONSTRAINT drafts_kind_check;
