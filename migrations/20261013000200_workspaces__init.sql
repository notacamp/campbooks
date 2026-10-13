-- The `workspaces` module: users, workspaces, members, invitations and the
-- security log. Workspace-owned tables are isolated by row-level security
-- (ADR 0011): every policy admits only the rows of the workspace set in the
-- transaction (`campbooks.workspace_id`, set by `begin_for` and by jobs), and
-- a few user-level reads also admit the signed-in user's own rows
-- (`campbooks.user_id`). `nullif(…, '')` because a setting a connection used
-- before reads as '' once its transaction ended; with nothing set, a query
-- sees no row instead of failing.
CREATE SCHEMA IF NOT EXISTS workspaces;

-- Everyone who has signed in, keyed by their identity-provider subject. Not
-- workspace-owned (a user belongs to any number of workspaces), so no
-- row-level security; only this module reads it.
CREATE TABLE workspaces.users (
    id uuid PRIMARY KEY,
    idp_subject text NOT NULL UNIQUE,
    display_name text,
    email text,
    locale text NOT NULL DEFAULT 'en' CHECK (locale IN ('en', 'pt-PT', 'es-ES', 'fr')),
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE workspaces.workspaces (
    id uuid PRIMARY KEY,
    name text NOT NULL CHECK (char_length(name) BETWEEN 1 AND 100),
    kind text NOT NULL CHECK (kind IN ('business', 'personal')),
    -- The design system's accent presets.
    accent text NOT NULL CHECK (accent IN ('business', 'personal', 'forest')),
    created_at timestamptz NOT NULL DEFAULT now(),
    -- Soft delete: hidden at once, purged by `workspaces.purge_workspace`
    -- once `purge_after` has passed.
    deleted_at timestamptz,
    purge_after timestamptz,
    CHECK ((deleted_at IS NULL) = (purge_after IS NULL))
);

CREATE TABLE workspaces.members (
    workspace_id uuid NOT NULL REFERENCES workspaces.workspaces (id) ON DELETE CASCADE,
    user_id uuid NOT NULL REFERENCES workspaces.users (id) ON DELETE CASCADE,
    role text NOT NULL CHECK (role IN ('owner', 'admin', 'member')),
    joined_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (workspace_id, user_id)
);
-- "My workspaces".
CREATE INDEX members_user_id_idx ON workspaces.members (user_id);

CREATE TABLE workspaces.invitations (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL REFERENCES workspaces.workspaces (id) ON DELETE CASCADE,
    -- Lower-cased.
    email text NOT NULL,
    role text NOT NULL CHECK (role IN ('admin', 'member')),
    -- SHA-256 of the token; the token itself is never stored.
    token_hash bytea NOT NULL UNIQUE,
    expires_at timestamptz NOT NULL,
    invited_by uuid NOT NULL REFERENCES workspaces.users (id),
    created_at timestamptz NOT NULL DEFAULT now(),
    accepted_at timestamptz,
    accepted_by uuid REFERENCES workspaces.users (id),
    CHECK ((accepted_at IS NULL) = (accepted_by IS NULL))
);
-- One pending invitation per address and workspace.
CREATE UNIQUE INDEX invitations_pending_idx ON workspaces.invitations (workspace_id, email)
    WHERE accepted_at IS NULL;

-- The security log. `workspace_id` is NULL for events about a user rather
-- than a workspace (first sign-in, signup requests). `metadata` holds ids and
-- roles, never tokens, emails or names.
CREATE TABLE workspaces.security_events (
    id uuid PRIMARY KEY,
    workspace_id uuid REFERENCES workspaces.workspaces (id) ON DELETE CASCADE,
    user_id uuid REFERENCES workspaces.users (id),
    kind text NOT NULL,
    at timestamptz NOT NULL DEFAULT now(),
    metadata jsonb NOT NULL DEFAULT '{}'
);
CREATE INDEX security_events_workspace_idx ON workspaces.security_events (workspace_id, at DESC);
CREATE INDEX security_events_user_idx ON workspaces.security_events (user_id, at DESC)
    WHERE workspace_id IS NULL;

-- Row-level security. FORCE applies it to the tables' owner too, for
-- deployments whose runtime login owns them.
ALTER TABLE workspaces.workspaces ENABLE ROW LEVEL SECURITY;
ALTER TABLE workspaces.workspaces FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON workspaces.workspaces
    USING (id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);
-- A user lists the workspaces they belong to.
CREATE POLICY members_see_their_workspaces ON workspaces.workspaces FOR SELECT
    USING (id IN (
        SELECT m.workspace_id FROM workspaces.members m
         WHERE m.user_id = nullif(current_setting('campbooks.user_id', true), '')::uuid
    ));

ALTER TABLE workspaces.members ENABLE ROW LEVEL SECURITY;
ALTER TABLE workspaces.members FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON workspaces.members
    USING (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);
-- A user reads their own memberships, across workspaces.
CREATE POLICY users_see_their_memberships ON workspaces.members FOR SELECT
    USING (user_id = nullif(current_setting('campbooks.user_id', true), '')::uuid);

ALTER TABLE workspaces.invitations ENABLE ROW LEVEL SECURITY;
ALTER TABLE workspaces.invitations FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON workspaces.invitations
    USING (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);

ALTER TABLE workspaces.security_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE workspaces.security_events FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON workspaces.security_events
    USING (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);
-- A user's own events that belong to no workspace (first sign-in, …).
CREATE POLICY users_own_instance_events ON workspaces.security_events
    USING (
        workspace_id IS NULL
        AND user_id = nullif(current_setting('campbooks.user_id', true), '')::uuid
    );

-- The runtime role (see `kernel__runtime_role`).
GRANT USAGE ON SCHEMA workspaces TO campbooks_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA workspaces TO campbooks_app;
