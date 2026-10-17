-- no-transaction
-- Every stream of an inbox at once (no `stream` filter): pinned first, then
-- newest first. (campbooks-cloud#165) Built CONCURRENTLY, one statement per
-- migration, for the reasons in 20261020235000_conversations__pinned_stream_list_index.sql.
CREATE INDEX CONCURRENTLY IF NOT EXISTS conversations_inbox_pinned_list_idx
    ON conversations.conversations (inbox_id, starred DESC, last_message_at DESC, id DESC)
    WHERE archived_at IS NULL AND NOT trashed;
