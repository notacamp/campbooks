-- no-transaction
-- Rule runs walk an inbox's conversations in id order. (campbooks-cloud#125)
-- Built CONCURRENTLY, so writes to the table go on during the build. That
-- cannot run in a transaction, and sqlx runs a migration file as one query
-- (an implicit transaction for several statements), so each index has a
-- migration of its own with a single statement. A build that fails or whose
-- process dies leaves an INVALID index of that name: before migrating, the
-- server drops invalid indexes (`migrate` in crates/server/src/main.rs, under
-- the migration lock, so a build in flight is never taken for one). IF NOT
-- EXISTS keeps a VALID index built just before the process died, before sqlx
-- recorded the migration.
CREATE INDEX CONCURRENTLY IF NOT EXISTS conversations_inbox_id_idx
    ON conversations.conversations (inbox_id, id);
