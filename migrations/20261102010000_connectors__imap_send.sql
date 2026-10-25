-- Sending from IMAP mailboxes over SMTP (imap-smtp-send, campbooks-cloud#313):
-- what became of a sent message's copy in the mailbox's Sent folder. The
-- send job records it `pending` once the SMTP server accepted the message;
-- the stream's next run appends it (or finds it there) and records how it
-- ended. No address and no content here: a state, a stable code, a count
-- and a blob reference (the bytes to append, held while pending).
--
-- `connectors.sends` keeps its row-level security (enabled and forced, the
-- `workspace_isolation` policy) and its grants; the new columns are its rows'.
--
-- Wait at most 5 s for a lock: a busy instance fails this migration fast,
-- and the next start retries it, rather than stalling.
SET LOCAL lock_timeout = '5s';

ALTER TABLE connectors.sends
    -- NULL for Gmail and Zoho sends, and every send before this change.
    ADD COLUMN sent_copy text CHECK (
        sent_copy IN ('pending', 'appended', 'found', 'provider', 'skipped', 'failed')
    ),
    -- The bytes to append (a blob of the workspace), while pending only.
    ADD COLUMN sent_copy_raw jsonb,
    -- Why the copy was skipped or failed: a stable code.
    ADD COLUMN sent_copy_reason text CHECK (sent_copy_reason ~ '^[a-z_]{1,40}$'),
    -- Runs that met trouble appending it.
    ADD COLUMN sent_copy_attempts integer NOT NULL DEFAULT 0 CHECK (sent_copy_attempts >= 0),
    ADD CONSTRAINT sends_sent_copy_raw_pending CHECK (
        sent_copy_raw IS NULL OR sent_copy = 'pending'
    );

-- The copies a stream's run still has to make, and the sent messages the
-- reader keeps although no copy of them is in the mailbox.
CREATE INDEX sends_unplaced_copy_idx ON connectors.sends (stream_id, provider_message_id)
    WHERE sent_copy IN ('pending', 'skipped', 'failed');
