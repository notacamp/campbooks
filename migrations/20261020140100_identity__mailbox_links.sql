-- The mailbox connections a way in brought, so that a way in that came only
-- from connecting a mailbox goes when its last such connection does
-- (campbooks-cloud#117). Recorded only for ways in a connect created (and
-- every later connection of them), so a way in linked before this table
-- existed, whose connections are not all known, is never removed this way.
-- The connection id is the connectors module's, by reference only. Account
-- data, not a workspace's: no workspace column, so the check for "another
-- connection still uses it" sees them all.
CREATE TABLE IF NOT EXISTS identity.mailbox_links (
    connection_id text PRIMARY KEY,
    provider_identity_id uuid NOT NULL
        REFERENCES identity.provider_identities (id) ON DELETE CASCADE,
    created_at timestamptz NOT NULL
);

CREATE INDEX IF NOT EXISTS mailbox_links_identity_idx
    ON identity.mailbox_links (provider_identity_id);

GRANT SELECT, INSERT, UPDATE, DELETE ON identity.mailbox_links TO campbooks_app;
