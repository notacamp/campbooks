-- The `assistant` module owns the `assistant` schema and nothing else: what
-- each member told Scout about how they want it to act. Other modules'
-- records are referenced by ID only: no cross-schema joins or foreign keys
-- (checked by `cargo xtask arch`).
-- Only new objects are created here; the timeout keeps an unexpected lock
-- wait from holding up a deploy.
SET LOCAL lock_timeout = '5s';

CREATE SCHEMA IF NOT EXISTS assistant;

-- One row per member and workspace, private to that member: every query
-- filters on the caller's own user id, and row-level security keeps
-- workspaces apart.
CREATE TABLE assistant.profiles (
    workspace_id uuid NOT NULL,
    -- A user id (workspaces module).
    user_id uuid NOT NULL,
    -- Whether rules Scout proposes from this member's corrections start
    -- active in a personal inbox (`act_then_tell`) or wait (`ask_first`).
    proactivity text NOT NULL DEFAULT 'ask_first'
        CHECK (proactivity IN ('ask_first', 'act_then_tell')),
    -- When the questions were answered or put off; NULL until then.
    asked_at timestamptz,
    -- Grows by one on every change.
    revision integer NOT NULL DEFAULT 1 CHECK (revision > 0),
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (workspace_id, user_id)
);

ALTER TABLE assistant.profiles ENABLE ROW LEVEL SECURITY;
ALTER TABLE assistant.profiles FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON assistant.profiles
    USING (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);

GRANT USAGE ON SCHEMA assistant TO campbooks_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA assistant TO campbooks_app;
