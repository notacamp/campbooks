-- no-transaction
-- A member's corrections of one message (`conversations.list_senders` with
-- `reachable`: did a member say this message is safe; campbooks-cloud#246).
-- Built CONCURRENTLY, one statement per migration, like
-- `messages_inbound_recent_idx`.
CREATE INDEX CONCURRENTLY IF NOT EXISTS corrections_message_idx
    ON conversations.corrections (message_id, question)
    WHERE message_id IS NOT NULL;
