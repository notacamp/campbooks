-- Any mailbox over IMAP and SMTP (imap-connection, campbooks-cloud#310): a
-- third provider, connected with the mailbox's password (usually an app
-- password) instead of an OAuth grant.
--
-- Wait at most 5 s for a lock: a busy instance fails this migration fast,
-- and the next start retries it, rather than stalling.
SET LOCAL lock_timeout = '5s';

ALTER TABLE connectors.connections DROP CONSTRAINT IF EXISTS connections_provider_check;
ALTER TABLE connectors.connections
    ADD CONSTRAINT connections_provider_check CHECK (provider IN ('google', 'zoho', 'imap'));

-- An IMAP connection keeps a password, sealed like a refresh token
-- (AES-256-GCM with the connectors key, associated data
-- `connectors.credentials:password:<connection id>`), and no tokens. A row
-- holds one or the other, never both.
ALTER TABLE connectors.credentials ALTER COLUMN refresh_token DROP NOT NULL;
ALTER TABLE connectors.credentials ADD COLUMN password bytea;
ALTER TABLE connectors.credentials
    ADD CONSTRAINT credentials_refresh_token_or_password
    CHECK ((refresh_token IS NULL) <> (password IS NULL));

-- Where an IMAP connection's mail lives: one row per IMAP connection. The
-- servers are the member's (found from the address, or typed in) and are
-- checked again each time they are reached. No secret here.
CREATE TABLE connectors.imap_servers (
    connection_id uuid PRIMARY KEY REFERENCES connectors.connections (id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL,
    -- What the mailbox signs in with (often its address).
    username text NOT NULL,
    imap_host text NOT NULL,
    imap_port integer NOT NULL CHECK (imap_port BETWEEN 1 AND 65535),
    imap_security text NOT NULL CHECK (imap_security IN ('tls', 'starttls')),
    smtp_host text NOT NULL,
    smtp_port integer NOT NULL CHECK (smtp_port BETWEEN 1 AND 65535),
    smtp_security text NOT NULL CHECK (smtp_security IN ('tls', 'starttls')),
    -- Where the settings came from.
    source text NOT NULL CHECK (source IN ('ispdb', 'autoconfig', 'srv', 'mx', 'manual')),
    -- The provider database's id of the provider (e.g. `yahoo.com`), when
    -- known: which app-password guide applies.
    provider_hint text,
    updated_at timestamptz NOT NULL
);

ALTER TABLE connectors.imap_servers ENABLE ROW LEVEL SECURITY;
ALTER TABLE connectors.imap_servers FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON connectors.imap_servers USING
    (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);

GRANT SELECT, INSERT, UPDATE, DELETE ON connectors.imap_servers TO campbooks_app;
