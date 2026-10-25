-- Checking an IMAP mailbox's address (imap-connect-screen, campbooks-cloud#314):
-- a mailbox whose username is not its address is sent a one-time code,
-- through its own SMTP server, which its stream's runs look for in its Inbox.
-- One row per connection: where its latest check is. No address, no code and
-- no content here: a state, a stable code, a nonce (the code is derived from
-- it with the instance key; alone it proves nothing), the member's locale
-- for the message, a count and times.
--
-- Wait at most 5 s for a lock: a busy instance fails this migration fast,
-- and the next start retries it, rather than stalling.
SET LOCAL lock_timeout = '5s';

CREATE TABLE connectors.address_checks (
    connection_id uuid PRIMARY KEY REFERENCES connectors.connections (id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL,
    state text NOT NULL CHECK (state IN ('sending', 'waiting', 'verified', 'failed')),
    -- Why it failed: a stable code.
    reason text CHECK (reason ~ '^[a-z_]{1,40}$'),
    nonce uuid NOT NULL,
    locale text NOT NULL CHECK (locale IN ('en', 'pt-PT', 'es-ES', 'fr')),
    -- Runs that looked for the code and did not find it.
    searches integer NOT NULL DEFAULT 0 CHECK (searches >= 0),
    sent_at timestamptz,
    updated_at timestamptz NOT NULL
);

ALTER TABLE connectors.address_checks ENABLE ROW LEVEL SECURITY;
ALTER TABLE connectors.address_checks FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON connectors.address_checks USING
    (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);

GRANT SELECT, INSERT, UPDATE, DELETE ON connectors.address_checks TO campbooks_app;
