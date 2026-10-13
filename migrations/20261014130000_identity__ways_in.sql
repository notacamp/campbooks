-- More ways in (ADR 0014, slice S2): a signed-in person adds a way in,
-- connects a mailbox or reconnects one through the same provider round trip
-- as sign-in.
--
-- A link ticket is what `identity.start_link` hands the browser: random,
-- stored as its SHA-256 only, single use, two minutes. It starts a round
-- trip only in a browser whose issuer session belongs to the same account.
-- `login_hint` (the address of a mailbox being reconnected) lives as long as
-- the ticket.
CREATE TABLE identity.link_tickets (
    ticket_hash bytea PRIMARY KEY,
    account_id uuid NOT NULL REFERENCES identity.accounts (id) ON DELETE CASCADE,
    provider text NOT NULL CHECK (provider IN ('google', 'microsoft', 'zoho')),
    purpose text NOT NULL CHECK (purpose IN ('link', 'connect', 'reconnect')),
    -- The workspace a mailbox is connected in, and the connection being
    -- reconnected: another module's, by id (no foreign key). Named `target_`
    -- because the row is a person's, not the workspace's (no row-level
    -- security, like every identity table).
    target_workspace_id uuid,
    connection_id text,
    -- A reconnect: the provider account (`sub`) of the connection. Only it
    -- may come back, and a reconnect never adds a way in.
    expected_subject text,
    login_hint text,
    return_to text NOT NULL,
    created_at timestamptz NOT NULL,
    expires_at timestamptz NOT NULL,
    CHECK ((purpose = 'link') = (target_workspace_id IS NULL)),
    CHECK ((purpose = 'reconnect') = (connection_id IS NOT NULL)),
    CHECK ((purpose = 'reconnect') = (expected_subject IS NOT NULL))
);
CREATE INDEX link_tickets_expires_idx ON identity.link_tickets (expires_at);

-- What a signed-in round trip is for.
ALTER TABLE identity.upstream_flows
    ADD COLUMN account_id uuid REFERENCES identity.accounts (id) ON DELETE CASCADE,
    ADD COLUMN target_workspace_id uuid,
    ADD COLUMN connection_id text,
    ADD COLUMN expected_subject text,
    ADD CONSTRAINT upstream_flows_account_purpose_check
        CHECK ((purpose = 'sign_in') = (account_id IS NULL));

-- The runtime role (see `kernel__runtime_role`).
GRANT SELECT, INSERT, UPDATE, DELETE ON identity.link_tickets TO campbooks_app;
