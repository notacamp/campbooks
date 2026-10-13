-- The `identity` module (ADR 0014): accounts and their ways in, the
-- issuer's browser sessions, provider round trips, mailbox grants held until
-- a workspace exists, and the authorization server's codes, refresh-token
-- families and signing keys.
--
-- None of these tables is workspace-owned (an account belongs to a person,
-- not a workspace), so they carry no row-level security; only this module
-- reads them. Every token, code and state value is stored as its SHA-256
-- only. What must be read back (PKCE verifiers sent upstream, provider
-- tokens, private signing keys) is sealed with AES-256-GCM under a key
-- derived from CAMPBOOKS_SECRET_KEY, bound to its row and column.
CREATE SCHEMA IF NOT EXISTS identity;

-- A person. `id` is the `sub` of every token issued for them.
CREATE TABLE identity.accounts (
    id uuid PRIMARY KEY,
    display_name text,
    -- From the way in the account was created with; shown, never matched.
    primary_email text,
    primary_email_verified boolean NOT NULL DEFAULT false,
    created_at timestamptz NOT NULL,
    disabled_at timestamptz
);

-- A provider account that is a way in. Linked only by a completed round
-- trip, never by email address.
CREATE TABLE identity.provider_identities (
    id uuid PRIMARY KEY,
    account_id uuid NOT NULL REFERENCES identity.accounts (id) ON DELETE CASCADE,
    provider text NOT NULL CHECK (provider IN ('google', 'microsoft', 'zoho')),
    issuer text NOT NULL,
    subject text NOT NULL,
    -- What the provider said when it was linked; for display only.
    email_at_link text,
    email_verified_at_link boolean NOT NULL DEFAULT false,
    linked_via text NOT NULL CHECK (linked_via IN ('sign_up', 'link', 'connect')),
    linked_at timestamptz NOT NULL,
    last_used_at timestamptz,
    UNIQUE (provider, issuer, subject)
);
CREATE INDEX provider_identities_account_idx ON identity.provider_identities (account_id);

-- A browser signed in to the issuer (single sign-on for every client).
CREATE TABLE identity.sessions (
    id uuid PRIMARY KEY,
    token_hash bytea NOT NULL UNIQUE,
    account_id uuid NOT NULL REFERENCES identity.accounts (id) ON DELETE CASCADE,
    -- How the person proved who they are.
    method text NOT NULL CHECK (method IN ('provider', 'passkey')),
    provider_identity_id uuid REFERENCES identity.provider_identities (id) ON DELETE SET NULL,
    auth_time timestamptz NOT NULL,
    user_agent text,
    created_at timestamptz NOT NULL,
    last_seen_at timestamptz NOT NULL,
    idle_expires_at timestamptz NOT NULL,
    expires_at timestamptz NOT NULL,
    ended_at timestamptz
);
CREATE INDEX sessions_account_idx ON identity.sessions (account_id);
CREATE INDEX sessions_ends_idx ON identity.sessions (least(idle_expires_at, expires_at));

-- An /oauth2/authorize request waiting for the person to sign in.
CREATE TABLE identity.authorize_requests (
    id_hash bytea PRIMARY KEY,
    client_id text NOT NULL,
    redirect_uri text NOT NULL,
    scope text NOT NULL,
    state text,
    nonce text,
    code_challenge text NOT NULL,
    created_at timestamptz NOT NULL,
    expires_at timestamptz NOT NULL
);
CREATE INDEX authorize_requests_expires_idx ON identity.authorize_requests (expires_at);

-- One round trip to a provider, from /auth/start to /auth/callback.
-- Single use: the callback deletes the row it redeems.
CREATE TABLE identity.upstream_flows (
    state_hash bytea PRIMARY KEY,
    -- SHA-256 of the flow cookie: only the browser that started it ends it.
    browser_hash bytea NOT NULL,
    provider text NOT NULL,
    purpose text NOT NULL CHECK (purpose IN ('sign_in', 'link', 'connect', 'reconnect')),
    -- The authorize request to complete afterwards (its id hash), if any.
    request_hash bytea,
    return_to text NOT NULL,
    nonce text NOT NULL,
    code_verifier bytea NOT NULL,
    -- This round trip already asked the provider for consent again.
    consent_retry boolean NOT NULL DEFAULT false,
    created_at timestamptz NOT NULL,
    expires_at timestamptz NOT NULL
);
CREATE INDEX upstream_flows_expires_idx ON identity.upstream_flows (expires_at);

-- A provider identity no account uses yet, waiting for the person to
-- confirm a new account. Single use, bound to the browser.
CREATE TABLE identity.pending_sign_ups (
    id uuid PRIMARY KEY,
    token_hash bytea NOT NULL UNIQUE,
    provider text NOT NULL,
    issuer text NOT NULL,
    subject text NOT NULL,
    email text,
    email_verified boolean NOT NULL,
    display_name text,
    -- The provider's grant (sealed JSON: access and refresh tokens, scopes).
    grant_tokens bytea,
    request_hash bytea,
    return_to text NOT NULL,
    created_at timestamptz NOT NULL,
    expires_at timestamptz NOT NULL
);
CREATE INDEX pending_sign_ups_expires_idx ON identity.pending_sign_ups (expires_at);

-- A mailbox grant obtained at sign-up, waiting for a workspace to land in.
CREATE TABLE identity.held_grants (
    id uuid PRIMARY KEY,
    account_id uuid NOT NULL REFERENCES identity.accounts (id) ON DELETE CASCADE,
    provider_identity_id uuid NOT NULL
        REFERENCES identity.provider_identities (id) ON DELETE CASCADE,
    provider text NOT NULL,
    address text NOT NULL,
    display_name text,
    scopes text[] NOT NULL,
    grant_tokens bytea NOT NULL,
    created_at timestamptz NOT NULL,
    expires_at timestamptz NOT NULL,
    UNIQUE (provider_identity_id)
);
CREATE INDEX held_grants_account_idx ON identity.held_grants (account_id);
CREATE INDEX held_grants_expires_idx ON identity.held_grants (expires_at);

-- A refresh-token family: every token rotated from one sign-in.
CREATE TABLE identity.refresh_families (
    id uuid PRIMARY KEY,
    client_id text NOT NULL,
    account_id uuid NOT NULL REFERENCES identity.accounts (id) ON DELETE CASCADE,
    session_id uuid REFERENCES identity.sessions (id) ON DELETE SET NULL,
    scope text NOT NULL,
    auth_time timestamptz NOT NULL,
    created_at timestamptz NOT NULL,
    expires_at timestamptz NOT NULL,
    revoked_at timestamptz,
    revoke_reason text
);
CREATE INDEX refresh_families_session_idx ON identity.refresh_families (session_id);
CREATE INDEX refresh_families_expires_idx ON identity.refresh_families (expires_at);

CREATE TABLE identity.refresh_tokens (
    token_hash bytea PRIMARY KEY,
    family_id uuid NOT NULL REFERENCES identity.refresh_families (id) ON DELETE CASCADE,
    issued_at timestamptz NOT NULL,
    idle_expires_at timestamptz NOT NULL,
    -- Set when it was exchanged for its successor; using it again is reuse.
    rotated_at timestamptz
);
CREATE INDEX refresh_tokens_family_idx ON identity.refresh_tokens (family_id);

-- Authorization codes: single use, 60 seconds.
CREATE TABLE identity.authorization_codes (
    code_hash bytea PRIMARY KEY,
    client_id text NOT NULL,
    account_id uuid NOT NULL REFERENCES identity.accounts (id) ON DELETE CASCADE,
    session_id uuid NOT NULL REFERENCES identity.sessions (id) ON DELETE CASCADE,
    redirect_uri text NOT NULL,
    scope text NOT NULL,
    nonce text,
    code_challenge text NOT NULL,
    auth_time timestamptz NOT NULL,
    expires_at timestamptz NOT NULL,
    redeemed_at timestamptz,
    -- The family the redemption created, revoked if the code is replayed.
    family_id uuid REFERENCES identity.refresh_families (id) ON DELETE SET NULL
);
CREATE INDEX authorization_codes_expires_idx ON identity.authorization_codes (expires_at);

-- The issuer's signing keys. Published from `publish_at` until
-- `delete_after`; signs from `activate_at` until `retire_at`.
CREATE TABLE identity.signing_keys (
    kid text PRIMARY KEY,
    alg text NOT NULL CHECK (alg = 'EdDSA'),
    public_jwk jsonb NOT NULL,
    private_key bytea NOT NULL,
    created_at timestamptz NOT NULL,
    publish_at timestamptz NOT NULL,
    activate_at timestamptz NOT NULL,
    retire_at timestamptz NOT NULL,
    delete_after timestamptz NOT NULL,
    CHECK (publish_at <= activate_at AND activate_at < retire_at AND retire_at < delete_after)
);

-- What happened to an account. `metadata` holds ids, provider names and
-- methods, never emails, tokens or secrets.
CREATE TABLE identity.audit_events (
    id uuid PRIMARY KEY,
    account_id uuid REFERENCES identity.accounts (id) ON DELETE CASCADE,
    kind text NOT NULL,
    at timestamptz NOT NULL,
    metadata jsonb NOT NULL DEFAULT '{}'
);
CREATE INDEX audit_events_account_idx ON identity.audit_events (account_id, at DESC);

-- The runtime role (see `kernel__runtime_role`).
GRANT USAGE ON SCHEMA identity TO campbooks_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA identity TO campbooks_app;
