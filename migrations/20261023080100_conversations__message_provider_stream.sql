-- Streams mirrored to the provider (provider-stream-labels,
-- campbooks-cloud#292).
--
-- `provider_stream`: the stream a received message is in at the provider,
-- as its last upsert said (`MessageChangeBatch` v1 `provider_stream`): one of
-- the four streams, `none` when it is in none of them, or NULL when no upsert
-- said (mail stored before this change, or from a connector that cannot
-- tell). When it changes from one said value to another for a known message,
-- the member moved it in their mail client and its conversation follows; a
-- first value moves nothing.
--
-- `mirrors_stream`: whether the module asks the provider to move the message
-- to its conversation's stream. Set for a received message stored from a
-- batch that is not its inbox's first sync (`backfill`) and that says a
-- provider stream; false for every message stored before this change, so
-- existing mail is never relabelled.
--
-- Both are added without a default that rewrites the table. The check is
-- added NOT VALID, so the swap holds the table's lock only for a moment; it is
-- validated by `…080200_conversations__message_provider_stream_validate`.
--
-- Wait at most 5 s for a lock: a busy instance fails this migration fast,
-- and the next start retries it, rather than stalling.
SET LOCAL lock_timeout = '5s';

ALTER TABLE conversations.messages ADD COLUMN IF NOT EXISTS provider_stream text;
ALTER TABLE conversations.messages ADD COLUMN IF NOT EXISTS mirrors_stream boolean NOT NULL DEFAULT false;
ALTER TABLE conversations.messages DROP CONSTRAINT IF EXISTS messages_provider_stream_check;
ALTER TABLE conversations.messages ADD CONSTRAINT messages_provider_stream_check
    CHECK (provider_stream IN ('primary', 'updates', 'receipts', 'newsletters', 'none')) NOT VALID;
