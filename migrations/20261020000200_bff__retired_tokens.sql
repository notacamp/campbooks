-- Every token a web session's cookie held before the last two, kept (as its
-- SHA-256) until the session ends: one presented again means the cookie was
-- copied, however long ago, and the session ends. At most one row per
-- rotation, so a session holds at most its lifetime over the rotation period
-- (about 720 with the defaults); they go with the session.
CREATE TABLE bff.retired_tokens (
    token_hash bytea PRIMARY KEY,
    session_id uuid NOT NULL REFERENCES bff.sessions (id) ON DELETE CASCADE,
    retired_at timestamptz NOT NULL
);
CREATE INDEX retired_tokens_session_idx ON bff.retired_tokens (session_id);

GRANT SELECT, INSERT, UPDATE, DELETE ON bff.retired_tokens TO campbooks_app;
