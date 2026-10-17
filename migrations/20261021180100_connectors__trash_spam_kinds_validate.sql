-- Validate the check `…180000_connectors__trash_spam_kinds` widened NOT VALID
-- (campbooks-cloud#169). A migration of its own, so it runs in a transaction
-- of its own: VALIDATE CONSTRAINT takes only SHARE UPDATE EXCLUSIVE, so reads
-- and writes go on while it scans the rows.

-- Never wait long for a lock on a busy table: fail and retry the deploy.
SET LOCAL lock_timeout = '5s';

ALTER TABLE connectors.outbound_changes VALIDATE CONSTRAINT outbound_changes_kind_check;
