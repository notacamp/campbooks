-- The mailboxes a way in brought, so that a way in that came only from
-- connecting a mailbox goes when its last such mailbox does
-- (campbooks-cloud#117). One row per workspace the account's mailbox is
-- connected in (the connectors module keeps one connection per workspace,
-- provider and account), written in the transaction that creates the way in,
-- or before the grant is handed over: if it cannot be written, nothing is.
-- Recorded only for ways in a connect created (and every later connection of
-- them), so a way in linked before this table existed, whose connections are
-- not all known, is never removed this way.
--
-- Account data, not a workspace's (like `consents.bound_workspace_id`): no
-- row-level security, so "another workspace still uses it" sees them all.
-- The workspace and connection are other modules' ids, by reference only;
-- the connection id is filled in once the connectors module answers, and
-- serves only to show which mailbox a way in came with.
CREATE TABLE IF NOT EXISTS identity.mailbox_links (
    provider_identity_id uuid NOT NULL
        REFERENCES identity.provider_identities (id) ON DELETE CASCADE,
    in_workspace_id uuid NOT NULL,
    connection_id text,
    created_at timestamptz NOT NULL,
    PRIMARY KEY (provider_identity_id, in_workspace_id)
);

GRANT SELECT, INSERT, UPDATE, DELETE ON identity.mailbox_links TO campbooks_app;
