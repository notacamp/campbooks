-- no-transaction
-- Spam (campbooks-cloud#240): binned conversations reported as spam, newest
-- first, keyset-paginated on (last_message_at, id). Built CONCURRENTLY, one
-- statement per migration, for the reasons in
-- 20261020235000_conversations__pinned_stream_list_index.sql.
CREATE INDEX CONCURRENTLY IF NOT EXISTS conversations_spam_list_idx
    ON conversations.conversations (inbox_id, last_message_at DESC, id DESC)
    WHERE trashed AND spam;
