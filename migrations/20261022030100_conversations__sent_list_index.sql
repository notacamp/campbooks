-- no-transaction
-- The Sent list of an inbox: your latest message first, keyset-paginated on
-- (last_sent_at, id). (campbooks-cloud#240) Built CONCURRENTLY, one statement
-- per migration, like the other list indexes.
CREATE INDEX CONCURRENTLY IF NOT EXISTS conversations_sent_list_idx
    ON conversations.conversations (inbox_id, last_sent_at DESC, id DESC)
    WHERE last_sent_at IS NOT NULL AND NOT trashed;
