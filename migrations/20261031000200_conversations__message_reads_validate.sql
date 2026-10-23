-- Validate the checks `…000100_conversations__message_reads` added NOT VALID
-- (campbooks-cloud#42). A migration of its own, so it runs in a transaction
-- of its own: VALIDATE CONSTRAINT takes only SHARE UPDATE EXCLUSIVE, so reads
-- and writes go on while it scans the rows.

-- Never wait long for a lock on a busy table: fail and retry the deploy.
SET LOCAL lock_timeout = '5s';

ALTER TABLE conversations.message_reads VALIDATE CONSTRAINT message_reads_state_check;
ALTER TABLE conversations.message_reads VALIDATE CONSTRAINT message_reads_layer_check;
ALTER TABLE conversations.message_reads VALIDATE CONSTRAINT message_reads_triggered_by_check;
ALTER TABLE conversations.message_reads VALIDATE CONSTRAINT message_reads_texts_check;
ALTER TABLE conversations.message_reads VALIDATE CONSTRAINT message_reads_items_check;
ALTER TABLE conversations.message_reads VALIDATE CONSTRAINT message_reads_asks_check;
ALTER TABLE conversations.message_reads VALIDATE CONSTRAINT message_reads_attempts_check;
