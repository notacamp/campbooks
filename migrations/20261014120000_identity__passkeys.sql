-- Passkeys (ADR 0014, slice S3): WebAuthn credentials as ways in, and the
-- single-use challenges of their ceremonies.
--
-- Like the rest of the `identity` schema these belong to a person, not a
-- workspace, so they carry no row-level security; only the identity module
-- reads them. A passkey's public key is not a secret. Challenges are stored
-- as their SHA-256 only.

-- A passkey: a discoverable WebAuthn credential registered with user
-- verification. `credential_id` is unique across the instance, so an
-- assertion names exactly one account.
CREATE TABLE identity.passkeys (
    id uuid PRIMARY KEY,
    account_id uuid NOT NULL REFERENCES identity.accounts (id) ON DELETE CASCADE,
    credential_id bytea NOT NULL UNIQUE
        CHECK (octet_length(credential_id) BETWEEN 16 AND 1023),
    -- The credential's COSE_Key, exactly as the authenticator sent it.
    public_key bytea NOT NULL,
    -- COSE algorithm: EdDSA (-8), ES256 (-7) or RS256 (-257).
    alg integer NOT NULL CHECK (alg IN (-8, -7, -257)),
    sign_count bigint NOT NULL DEFAULT 0 CHECK (sign_count >= 0),
    aaguid uuid NOT NULL,
    transports text[] NOT NULL DEFAULT '{}',
    -- The BE and BS flags: whether it may be synced, and whether it is.
    backup_eligible boolean NOT NULL,
    backup_state boolean NOT NULL,
    name text NOT NULL CHECK (char_length(name) BETWEEN 1 AND 64),
    created_at timestamptz NOT NULL,
    last_used_at timestamptz
);
CREATE INDEX passkeys_account_idx ON identity.passkeys (account_id);

-- One WebAuthn ceremony: a challenge handed to the browser, redeemed once
-- within five minutes.
--   register: a signed-in account adds a passkey (bound to the account);
--   sign_in:  someone signs in (bound to the browser that asked);
--   sign_up:  someone creates an account with a passkey alone (bound to the
--             browser; `user_handle` is the new account's id).
CREATE TABLE identity.webauthn_challenges (
    challenge_hash bytea PRIMARY KEY,
    purpose text NOT NULL CHECK (purpose IN ('register', 'sign_in', 'sign_up')),
    account_id uuid REFERENCES identity.accounts (id) ON DELETE CASCADE,
    -- SHA-256 of the ceremony cookie.
    browser_hash bytea,
    user_handle uuid,
    display_name text,
    -- The authorization request to complete afterwards, if any.
    request_id text,
    return_to text,
    created_at timestamptz NOT NULL,
    expires_at timestamptz NOT NULL,
    CHECK (
        (purpose = 'register' AND account_id IS NOT NULL)
        OR (purpose = 'sign_in' AND browser_hash IS NOT NULL AND return_to IS NOT NULL)
        OR (purpose = 'sign_up' AND browser_hash IS NOT NULL AND return_to IS NOT NULL
            AND user_handle IS NOT NULL AND display_name IS NOT NULL)
    )
);
CREATE INDEX webauthn_challenges_expires_idx ON identity.webauthn_challenges (expires_at);

-- A session started with a passkey names it, and every session records the
-- authentication methods its tokens report (`amr`, RFC 8176).
ALTER TABLE identity.sessions
    ADD COLUMN passkey_id uuid REFERENCES identity.passkeys (id) ON DELETE SET NULL,
    ADD COLUMN amr text[] NOT NULL DEFAULT '{fed}';

GRANT SELECT, INSERT, UPDATE, DELETE ON identity.passkeys, identity.webauthn_challenges
    TO campbooks_app;
