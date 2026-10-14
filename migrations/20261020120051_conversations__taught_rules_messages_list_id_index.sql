-- no-transaction
-- Rules by mailing list. (campbooks-cloud#125)
-- Built CONCURRENTLY, so writes to the table go on during the build. That
-- cannot run in a transaction, and sqlx runs a migration file as one query
-- (an implicit transaction for several statements), so each index has a
-- migration of its own with a single statement.
CREATE INDEX CONCURRENTLY IF NOT EXISTS messages_list_id_idx
    ON conversations.messages (inbox_id, (headers ->> 'list_id'))
    WHERE direction = 'inbound' AND headers ? 'list_id';
