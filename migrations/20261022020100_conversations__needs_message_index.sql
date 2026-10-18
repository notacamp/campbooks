-- no-transaction
-- A need by the message it comes from (`conversations.list_senders` with
-- `reachable`: is this message under review; campbooks-cloud#246). Built
-- CONCURRENTLY, one statement per migration, like `messages_inbound_recent_idx`.
CREATE INDEX CONCURRENTLY IF NOT EXISTS needs_message_idx
    ON conversations.needs (message_id);
