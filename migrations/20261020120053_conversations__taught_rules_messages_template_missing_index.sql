-- no-transaction
-- Older mail still to get its subject template (`conversations.fill_templates`). (campbooks-cloud#125)
-- Built CONCURRENTLY, so writes to the table go on during the build. That
-- cannot run in a transaction, and sqlx runs a migration file as one query
-- (an implicit transaction for several statements), so each index has a
-- migration of its own with a single statement.
CREATE INDEX CONCURRENTLY IF NOT EXISTS messages_template_missing_idx
    ON conversations.messages (inbox_id)
    WHERE direction = 'inbound' AND subject_template IS NULL;
