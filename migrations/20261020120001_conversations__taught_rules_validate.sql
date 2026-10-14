-- Validate the checks `…120000_conversations__taught_rules` widened NOT VALID.
-- (campbooks-cloud#125) A migration of its own, so it runs in a transaction
-- of its own: VALIDATE CONSTRAINT takes only SHARE UPDATE EXCLUSIVE, so
-- reads and writes go on while it scans the rows.

-- Never wait long for a lock on a busy table: fail and retry the deploy.
SET LOCAL lock_timeout = '5s';

ALTER TABLE conversations.conversations VALIDATE CONSTRAINT conversations_stream_source_check;
ALTER TABLE conversations.decisions VALIDATE CONSTRAINT decisions_source_check;
ALTER TABLE conversations.needs VALIDATE CONSTRAINT needs_basis_check;
ALTER TABLE conversations.needs VALIDATE CONSTRAINT needs_cleared_because_check;
