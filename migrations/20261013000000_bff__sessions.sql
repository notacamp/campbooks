-- The web BFF's session store (ADR 0004). The `bff` schema belongs to
-- `campbooks-server`'s backend-for-frontend, not to a module; module
-- migrations may not touch it (`cargo xtask arch`).
--
-- Cookies hold random 256-bit tokens; only their SHA-256 is stored here.
-- Access, refresh and ID tokens are AES-256-GCM ciphertexts under a key
-- derived from CAMPBOOKS_SECRET_KEY, bound to the row id and column.
CREATE SCHEMA IF NOT EXISTS bff;

-- One signed-in browser.
CREATE TABLE bff.sessions (
    id uuid PRIMARY KEY,
    -- SHA-256 of the session cookie's token.
    token_hash bytea NOT NULL UNIQUE,
    -- The token it replaced at the last rotation, accepted for a short grace
    -- period so requests in flight during the rotation still succeed.
    previous_token_hash bytea UNIQUE,
    rotated_at timestamptz NOT NULL,
    -- The user at the IdP (`sub`) and the IdP's session (`sid`), matched by
    -- back-channel logout.
    subject text NOT NULL,
    idp_session_id text,
    -- From the ID token, for display.
    name text,
    email text,
    access_token bytea NOT NULL,
    access_expires_at timestamptz NOT NULL,
    refresh_token bytea,
    id_token bytea NOT NULL,
    -- The browser's User-Agent at sign-in (truncated), to tell sessions apart.
    user_agent text,
    created_at timestamptz NOT NULL,
    last_seen_at timestamptz NOT NULL,
    -- Ends when unused until then (moves with `last_seen_at`) …
    idle_expires_at timestamptz NOT NULL,
    -- … and at the latest at this absolute deadline.
    expires_at timestamptz NOT NULL
);

CREATE INDEX sessions_subject_idx ON bff.sessions (subject);
CREATE INDEX sessions_idp_session_id_idx ON bff.sessions (idp_session_id)
    WHERE idp_session_id IS NOT NULL;
-- Pruning finds ended sessions.
CREATE INDEX sessions_ends_at_idx ON bff.sessions (least(idle_expires_at, expires_at));

-- A sign-in started at /bff/login and finished at /bff/callback. Single use:
-- the callback deletes the row it redeems.
CREATE TABLE bff.login_transactions (
    -- SHA-256 of the `state` sent to the IdP.
    state_hash bytea PRIMARY KEY,
    -- SHA-256 of the login cookie's token: only the browser that started the
    -- sign-in can finish it.
    browser_hash bytea NOT NULL,
    nonce text NOT NULL,
    -- The PKCE verifier, encrypted like the session tokens.
    code_verifier bytea NOT NULL,
    -- Same-origin path to land on afterwards.
    return_to text NOT NULL,
    created_at timestamptz NOT NULL,
    expires_at timestamptz NOT NULL
);

CREATE INDEX login_transactions_expires_at_idx ON bff.login_transactions (expires_at);
