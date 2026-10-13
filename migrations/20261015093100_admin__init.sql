-- The `admin` module (ADR 0015): who administers this instance, and what
-- they did.
--
-- Profiles are written only by the server's CLI (`campbooks-server admin
-- grant|revoke|list`), which connects with the migration login. The runtime
-- role every request runs as may read profiles and append to the trail,
-- nothing else, so no request handler can make someone an administrator,
-- even by mistake. Not workspace-owned: no row-level security.
CREATE SCHEMA IF NOT EXISTS admin;

-- One administrator. `account_id` is the identity account id (the token
-- `sub`), referenced by id only. Each permission is a
-- `kernel::admin::Permission` wire name (`super`, `system.read`, ...),
-- checked by the CLI.
CREATE TABLE admin.profiles (
    account_id text PRIMARY KEY,
    permissions text[] NOT NULL CHECK (cardinality(permissions) > 0),
    note text,
    granted_at timestamptz NOT NULL,
    updated_at timestamptz NOT NULL
);

-- The audit trail: every admin API request that passed the profile check
-- (`via = 'api'`, with its response status) and every CLI change
-- (`via = 'cli'`). Append-only for the runtime role.
CREATE TABLE admin.actions (
    id uuid PRIMARY KEY,
    at timestamptz NOT NULL,
    actor text NOT NULL,
    via text NOT NULL CHECK (via IN ('api', 'cli')),
    operation text NOT NULL,
    target jsonb NOT NULL DEFAULT '{}'::jsonb,
    status smallint
);
CREATE INDEX actions_at_idx ON admin.actions (at DESC);
CREATE INDEX actions_actor_idx ON admin.actions (actor, at DESC);

GRANT USAGE ON SCHEMA admin TO campbooks_app;
GRANT SELECT ON admin.profiles TO campbooks_app;
GRANT INSERT ON admin.actions TO campbooks_app;
