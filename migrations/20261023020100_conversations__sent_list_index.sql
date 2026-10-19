-- no-transaction
-- Sent (campbooks-cloud#240): every conversation the inbox wrote in, outside
-- spam and the trash, in any stream, archived or not, by the inbox's latest
-- message, keyset-paginated on (last_sent_at, id). Built CONCURRENTLY, one
-- statement per migration, for the reasons in
-- 20261020235000_conversations__pinned_stream_list_index.sql.
CREATE INDEX CONCURRENTLY IF NOT EXISTS conversations_sent_list_idx
    ON conversations.conversations (inbox_id, last_sent_at DESC, id DESC)
    WHERE last_sent_at IS NOT NULL AND NOT trashed;
