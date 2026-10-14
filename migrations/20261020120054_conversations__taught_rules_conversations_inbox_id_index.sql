-- no-transaction
-- Rule runs walk an inbox's conversations in id order. (campbooks-cloud#125)
-- Built CONCURRENTLY, so writes to the table go on during the build. That
-- cannot run in a transaction, and sqlx runs a migration file as one query
-- (an implicit transaction for several statements), so each index has a
-- migration of its own with a single statement. sqlx records it only once
-- it succeeds; if a build fails partway, Postgres keeps an INVALID index of
-- that name, so the re-run fails loudly ("already exists") rather than
-- keeping it. Recovery: DROP INDEX CONCURRENTLY <name>; then restart.
CREATE INDEX CONCURRENTLY conversations_inbox_id_idx
    ON conversations.conversations (inbox_id, id);
