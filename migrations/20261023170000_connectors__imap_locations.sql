-- Reading IMAP mailboxes (imap-messages-adapter, campbooks-cloud#311): where
-- each message of an IMAP stream is. One message (`message_key`, the index's
-- provider message id: its normalised Message-ID, or a digest of its headers)
-- can be in several folders; each copy is a location (folder, UIDVALIDITY,
-- UID), kept for the message's state (read, flagged, which folders) and for
-- write-back (campbooks-cloud#312). No message content here: the folder name
-- as the server spells it, numbers, two flags and the server's own id.
--
-- Wait at most 5 s for a lock: a busy instance fails this migration fast,
-- and the next start retries it, rather than stalling.
SET LOCAL lock_timeout = '5s';

CREATE TABLE connectors.imap_locations (
    workspace_id uuid NOT NULL,
    stream_id uuid NOT NULL REFERENCES connectors.streams (id) ON DELETE CASCADE,
    -- The folder, as the server names it (modified UTF-7 kept raw).
    folder text NOT NULL,
    -- RFC 3501 UIDVALIDITY and UID: unsigned 32-bit numbers.
    uidvalidity bigint NOT NULL CHECK (uidvalidity BETWEEN 1 AND 4294967295),
    uid bigint NOT NULL CHECK (uid BETWEEN 1 AND 4294967295),
    -- The message: `connectors.messages.provider_message_id` of the stream.
    message_key text NOT NULL,
    -- `\Seen` and `\Flagged` on this copy.
    seen boolean NOT NULL DEFAULT false,
    flagged boolean NOT NULL DEFAULT false,
    -- The server's own id of the message (RFC 8474 EMAILID), when it gives
    -- one: tells apart two messages that share a Message-ID.
    email_id text,
    PRIMARY KEY (stream_id, folder, uidvalidity, uid)
);

CREATE INDEX imap_locations_message_idx ON connectors.imap_locations (stream_id, message_key);

ALTER TABLE connectors.imap_locations ENABLE ROW LEVEL SECURITY;
ALTER TABLE connectors.imap_locations FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON connectors.imap_locations USING
    (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);

GRANT SELECT, INSERT, UPDATE, DELETE ON connectors.imap_locations TO campbooks_app;
