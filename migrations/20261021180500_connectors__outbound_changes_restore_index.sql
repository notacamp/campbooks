-- no-transaction
-- The outbound changes still holding the Zoho folder a message was trashed
-- from (`restore_to`, campbooks-cloud#169): few, found per stream when a sync
-- sees a message leave the trash. Built CONCURRENTLY and apart from the
-- constraint swap of 20261021180000, so neither the build nor writes to the
-- table wait on that lock. That cannot run in a transaction, and sqlx runs a
-- migration file as one query, so the index has a migration of its own with
-- a single statement. A build that fails leaves an INVALID index of that
-- name, which the server drops before migrating (`migrate` in
-- crates/server/src/main.rs); IF NOT EXISTS keeps a VALID one built just
-- before the process died.
CREATE INDEX CONCURRENTLY IF NOT EXISTS outbound_changes_restore_idx
    ON connectors.outbound_changes (stream_id)
    WHERE restore_to IS NOT NULL;
