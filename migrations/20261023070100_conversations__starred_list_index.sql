-- no-transaction
-- Pinned (campbooks-cloud#277): starred conversations outside spam and the
-- trash, in any stream, archived or not, newest first, keyset-paginated on
-- (last_message_at, id). Few conversations are pinned, so without its own
-- index a page would walk the whole mailbox to find them. Built
-- CONCURRENTLY, one statement per migration, for the reasons in
-- 20261020235000_conversations__pinned_stream_list_index.sql.
CREATE INDEX CONCURRENTLY IF NOT EXISTS conversations_starred_list_idx
    ON conversations.conversations (inbox_id, last_message_at DESC, id DESC)
    WHERE starred AND NOT trashed;
