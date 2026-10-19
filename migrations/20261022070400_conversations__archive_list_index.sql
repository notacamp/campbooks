-- no-transaction
-- Archive (campbooks-cloud#240): conversations out of the inbox and outside
-- spam and the trash, newest first, keyset-paginated on (last_message_at,
-- id). `conversations_list_idx` covers every conversation not binned, so an
-- archive page there walks past the whole inbox first. Built CONCURRENTLY,
-- one statement per migration, for the reasons in
-- 20261020235000_conversations__pinned_stream_list_index.sql.
CREATE INDEX CONCURRENTLY IF NOT EXISTS conversations_archive_list_idx
    ON conversations.conversations (inbox_id, last_message_at DESC, id DESC)
    WHERE archived_at IS NOT NULL AND NOT trashed;
