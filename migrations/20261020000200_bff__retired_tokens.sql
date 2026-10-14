-- Fail fast rather than queue behind a long transaction while every request
-- that touches these tables waits behind the migration; the server's start
-- (and so the migration) is retried.
SET LOCAL lock_timeout = '5s';

-- Every token a web session's cookie held before the last two, kept (as its
-- SHA-256) until the session ends: one presented again means the cookie was
-- copied, however long ago, and the session ends. A row is added by a
-- rotation, or by a reissue (below), which waits for the two-minute grace:
-- at most about one row per two minutes of a session's life, usually one
-- per rotation period (an hour); they go with the session.
CREATE TABLE bff.retired_tokens (
    token_hash bytea PRIMARY KEY,
    session_id uuid NOT NULL REFERENCES bff.sessions (id) ON DELETE CASCADE,
    retired_at timestamptz NOT NULL
);
CREATE INDEX retired_tokens_session_idx ON bff.retired_tokens (session_id);

GRANT SELECT, INSERT, UPDATE, DELETE ON bff.retired_tokens TO campbooks_app;

-- Whether the session's current token has reached the browser: false from a
-- rotation until a request presents it. A token rotated away is a theft
-- signal only once its successor was used; until then the response carrying
-- the new cookie may simply have been lost (an aborted fetch, a reload, a
-- sleeping laptop), and the old token gets a fresh rotation instead.
ALTER TABLE bff.sessions ADD COLUMN token_delivered boolean NOT NULL DEFAULT true;
