-- Instance administration (ADR 0015, slice S2): an administrator's
-- transaction (`kernel::db::begin_admin`, which sets `campbooks.admin_reads`)
-- may read every workspace's metadata: the workspace itself, who belongs to
-- it with which role, and its invitations (only counted). Reads only: the
-- policies are `FOR SELECT`, so admin transactions change nothing here.
-- `workspaces.users` has no row-level security and needs none.
CREATE POLICY admin_reads ON workspaces.workspaces FOR SELECT
    USING (current_setting('campbooks.admin_reads', true) = 'on');

CREATE POLICY admin_reads ON workspaces.members FOR SELECT
    USING (current_setting('campbooks.admin_reads', true) = 'on');

CREATE POLICY admin_reads ON workspaces.invitations FOR SELECT
    USING (current_setting('campbooks.admin_reads', true) = 'on');

-- Admin → People pages through users newest first.
CREATE INDEX users_created_idx ON workspaces.users (created_at DESC, id DESC);
