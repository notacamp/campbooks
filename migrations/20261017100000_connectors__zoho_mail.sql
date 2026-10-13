-- Zoho Mail (zoho-mail-adapter): a second provider, several mailboxes per
-- connection, and one index of emitted messages shared by the adapters.

-- Zoho connections, and the accounts server of the data centre a Zoho grant
-- came from (the ID token's validated issuer): where its tokens are
-- refreshed and, by Zoho's fixed mapping, where its mail lives. NULL for
-- Google.
ALTER TABLE connectors.connections DROP CONSTRAINT IF EXISTS connections_provider_check;
ALTER TABLE connectors.connections
    ADD CONSTRAINT connections_provider_check CHECK (provider IN ('google', 'zoho'));
ALTER TABLE connectors.connections ADD COLUMN provider_server text;

-- One stream per capability and mailbox: a Zoho login can hold several mail
-- accounts (its `accountId`s), each read as its own stream into its own
-- inbox. Gmail has one mailbox per connection, keyed ''.
ALTER TABLE connectors.streams ADD COLUMN mailbox_key text NOT NULL DEFAULT '';
-- The mailbox's address (what the stream's inbox is called).
ALTER TABLE connectors.streams ADD COLUMN address text;
ALTER TABLE connectors.streams DROP CONSTRAINT IF EXISTS streams_connection_id_capability_key;
ALTER TABLE connectors.streams
    ADD CONSTRAINT streams_connection_capability_mailbox_key
    UNIQUE (connection_id, capability, mailbox_key);

-- The adapters' index of what they emitted (Gmail first, now Zoho too).
-- `label_ids` holds Gmail's label ids, or Zoho's state as tokens
-- (`folder:<id>`, `INBOX`, `SENT`, `SPAM`, `TRASH`, `UNREAD`, `STARRED`,
-- `ARCHIVED`). `rfc_message_id` lets a reply name the message it answers.
ALTER TABLE connectors.gmail_messages RENAME TO messages;
ALTER TABLE connectors.gmail_oversized RENAME TO oversized;
ALTER INDEX connectors.gmail_messages_window_idx RENAME TO messages_window_idx;
ALTER INDEX connectors.gmail_messages_reemit_idx RENAME TO messages_reemit_idx;
ALTER TABLE connectors.messages ADD COLUMN rfc_message_id text;
CREATE INDEX messages_rfc_idx ON connectors.messages (stream_id, rfc_message_id)
    WHERE rfc_message_id IS NOT NULL;

-- Messages the provider failed on, one at a time (a download or a look-up
-- answered with an error other than "gone"): skipped until `retry_at`, which
-- backs off with each failure, so one bad message never stops a mailbox.
CREATE TABLE connectors.skipped_messages (
    workspace_id uuid NOT NULL,
    stream_id uuid NOT NULL REFERENCES connectors.streams (id) ON DELETE CASCADE,
    provider_message_id text NOT NULL,
    failures integer NOT NULL DEFAULT 1,
    retry_at timestamptz NOT NULL,
    PRIMARY KEY (stream_id, provider_message_id)
);

ALTER TABLE connectors.skipped_messages ENABLE ROW LEVEL SECURITY;
ALTER TABLE connectors.skipped_messages FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON connectors.skipped_messages USING
    (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);

GRANT SELECT, INSERT, UPDATE, DELETE ON connectors.skipped_messages TO campbooks_app;
