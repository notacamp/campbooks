-- Validate the check `…080100_conversations__message_provider_stream` added
-- NOT VALID (campbooks-cloud#292). A migration of its own, so it runs in a
-- transaction of its own: VALIDATE CONSTRAINT takes only SHARE UPDATE
-- EXCLUSIVE, so reads and writes go on while it scans the rows.

-- Never wait long for a lock on a busy table: fail and retry the deploy.
SET LOCAL lock_timeout = '5s';

ALTER TABLE conversations.messages VALIDATE CONSTRAINT messages_provider_stream_check;
