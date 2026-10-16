-- no-transaction
-- An inbox's received mail, newest first: who writes to it lately
-- (`conversations.list_senders`, which reads only the last year, at most
-- 5,000 messages, so its cost doesn't grow with an inbox's history).
-- (campbooks-cloud#121)
-- Built CONCURRENTLY, so writes to the table go on during the build. That
-- cannot run in a transaction, and sqlx runs a migration file as one query
-- (an implicit transaction for several statements), so each index has a
-- migration of its own with a single statement. A build that fails or whose
-- process dies leaves an INVALID index of that name: before migrating, the
-- server drops invalid indexes (`migrate` in crates/server/src/main.rs, under
-- the migration lock, so a build in flight is never taken for one). IF NOT
-- EXISTS keeps a VALID index built just before the process died, before sqlx
-- recorded the migration.
CREATE INDEX CONCURRENTLY IF NOT EXISTS messages_inbound_recent_idx
    ON conversations.messages (inbox_id, sent_at DESC)
    WHERE direction = 'inbound' AND NOT in_spam AND NOT in_trash;
