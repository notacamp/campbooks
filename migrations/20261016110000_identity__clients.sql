-- Clients other than Campbooks' own, consents, and tokens bound to a
-- resource and a workspace (ADR 0014, slice S4).
--
-- `clients` holds a workspace's own integrations (workspace-owned: forced
-- row-level security, like any workspace table), clients registered
-- dynamically (RFC 7591) and clients known by their metadata document
-- (MCP's Client ID Metadata Documents); the last two belong to no
-- workspace. The authorization server must find any client by id before it
-- knows a workspace, so a second policy admits rows to transactions that
-- set `campbooks.identity_role = 'authorization_server'` (LOCAL, only in
-- the identity module's protocol code). Client secrets are stored as their
-- SHA-256 only.
CREATE TABLE identity.clients (
    client_id text PRIMARY KEY CHECK (char_length(client_id) BETWEEN 1 AND 2048),
    kind text NOT NULL CHECK (kind IN ('workspace', 'dynamic', 'metadata_document')),
    workspace_id uuid,
    name text NOT NULL CHECK (char_length(name) BETWEEN 1 AND 100),
    -- Shown on the consent screen for a workspace's own client only.
    logo_uri text,
    redirect_uris text[] NOT NULL DEFAULT '{}',
    grant_types text[] NOT NULL,
    auth_method text NOT NULL CHECK (auth_method IN ('none', 'client_secret_basic')),
    secret_hash bytea,
    allowed_scopes text[] NOT NULL,
    -- The account that registered a workspace client.
    created_by uuid,
    created_at timestamptz NOT NULL,
    secret_rotated_at timestamptz,
    last_used_at timestamptz,
    -- A metadata document's cache: fetch it again after this.
    metadata_expires_at timestamptz,
    CHECK ((kind = 'workspace') = (workspace_id IS NOT NULL)),
    CHECK ((auth_method = 'none') = (secret_hash IS NULL)),
    CHECK (kind = 'workspace' OR auth_method = 'none'),
    CHECK ((kind = 'metadata_document') = (metadata_expires_at IS NOT NULL))
);
CREATE INDEX clients_workspace_idx ON identity.clients (workspace_id);
CREATE INDEX clients_unused_idx ON identity.clients (kind, coalesce(last_used_at, created_at));

ALTER TABLE identity.clients ENABLE ROW LEVEL SECURITY;
ALTER TABLE identity.clients FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON identity.clients
    USING (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);
CREATE POLICY authorization_server ON identity.clients
    USING (current_setting('campbooks.identity_role', true) = 'authorization_server')
    WITH CHECK (current_setting('campbooks.identity_role', true) = 'authorization_server');

-- A person's yes to a client. At most one live consent per (account,
-- client); a new one replaces it. `remembered` consents let later
-- authorizations skip the screen (only for clients whose redirect is an
-- exact https URI). The bound workspace and member are what the person
-- chose: the row is theirs, not the workspace's (no row-level security).
CREATE TABLE identity.consents (
    id uuid PRIMARY KEY,
    account_id uuid NOT NULL REFERENCES identity.accounts (id) ON DELETE CASCADE,
    client_id text NOT NULL,
    bound_workspace_id uuid,
    -- The workspaces module's user id at grant time, so a member leaving
    -- revokes it.
    bound_member_id uuid,
    scopes text[] NOT NULL,
    remembered boolean NOT NULL,
    granted_at timestamptz NOT NULL,
    last_used_at timestamptz,
    revoked_at timestamptz
);
CREATE UNIQUE INDEX consents_live_idx ON identity.consents (account_id, client_id)
    WHERE revoked_at IS NULL;
CREATE INDEX consents_client_idx ON identity.consents (client_id);
CREATE INDEX consents_bound_workspace_idx ON identity.consents (bound_workspace_id);

-- The resource (RFC 8707) and the workspace a request, code and family are
-- for; and whether a stored request waits on the consent screen.
ALTER TABLE identity.authorize_requests
    ADD COLUMN resource text,
    -- The screen must be shown (`prompt=consent`, or the request waits on it).
    ADD COLUMN consent_due boolean NOT NULL DEFAULT false,
    -- The account a request waiting on the consent screen was put to: only
    -- that person may answer it.
    ADD COLUMN consent_account_id uuid REFERENCES identity.accounts (id) ON DELETE CASCADE;
ALTER TABLE identity.authorization_codes
    ADD COLUMN resource text,
    ADD COLUMN bound_workspace_id uuid;
ALTER TABLE identity.refresh_families
    ADD COLUMN resource text,
    ADD COLUMN bound_workspace_id uuid;
CREATE INDEX refresh_families_client_idx ON identity.refresh_families (client_id, account_id);

GRANT SELECT, INSERT, UPDATE, DELETE ON identity.clients, identity.consents TO campbooks_app;
