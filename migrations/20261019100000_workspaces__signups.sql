-- Signup requests and single-use signup codes (ADR 0015, admin slice
-- "Signups"). Both are instance-level, not workspace-owned.

-- One request per user to create their first workspace on an instance whose
-- signup policy is `approval`. `decided_by` is the deciding administrator's
-- identity account id (referenced by id only); it is never shown to the
-- requester.
CREATE TABLE workspaces.signup_requests (
    user_id uuid PRIMARY KEY REFERENCES workspaces.users (id) ON DELETE CASCADE,
    status text NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'approved', 'rejected')),
    requested_at timestamptz NOT NULL DEFAULT now(),
    decided_at timestamptz,
    decided_by text,
    reason text CHECK (char_length(reason) <= 500),
    CHECK ((status = 'pending') = (decided_at IS NULL))
);
-- The admin list: pending first, then newest first.
CREATE INDEX signup_requests_status_idx ON workspaces.signup_requests (status, requested_at DESC);

ALTER TABLE workspaces.signup_requests ENABLE ROW LEVEL SECURITY;
ALTER TABLE workspaces.signup_requests FORCE ROW LEVEL SECURITY;
-- A user sees, and files, their own request.
CREATE POLICY own_request ON workspaces.signup_requests FOR SELECT
    USING (user_id = nullif(current_setting('campbooks.user_id', true), '')::uuid);
CREATE POLICY own_request_filed ON workspaces.signup_requests FOR INSERT
    WITH CHECK (
        status = 'pending'
        AND user_id = nullif(current_setting('campbooks.user_id', true), '')::uuid
    );
-- An administrator reads every request (metadata only: ids, status, times)
-- and decides it.
CREATE POLICY admin_reads_requests ON workspaces.signup_requests FOR SELECT
    USING (current_setting('campbooks.admin_reads', true) = 'on');
CREATE POLICY admin_decides_requests ON workspaces.signup_requests FOR UPDATE
    USING (current_setting('campbooks.admin_reads', true) = 'on')
    WITH CHECK (current_setting('campbooks.admin_reads', true) = 'on');

-- Codes an administrator mints under the `code` policy. Only the SHA-256 of
-- a code is stored: the plaintext is shown once, at mint. A code is
-- redeemed by one atomic `UPDATE ... SET uses = uses + 1` in the
-- transaction that creates the workspace. Not workspace-owned and holding
-- no personal data, so no row-level security (like `workspaces.users`);
-- only this module reads it.
CREATE TABLE workspaces.signup_codes (
    id uuid PRIMARY KEY,
    digest bytea NOT NULL UNIQUE,
    label text NOT NULL CHECK (char_length(label) BETWEEN 1 AND 100),
    max_uses integer NOT NULL DEFAULT 1 CHECK (max_uses BETWEEN 1 AND 100000),
    uses integer NOT NULL DEFAULT 0 CHECK (uses >= 0 AND uses <= max_uses),
    expires_at timestamptz,
    created_by text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    revoked_at timestamptz
);
CREATE INDEX signup_codes_created_idx ON workspaces.signup_codes (created_at DESC, id DESC);

GRANT SELECT, INSERT, UPDATE, DELETE ON workspaces.signup_requests TO campbooks_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON workspaces.signup_codes TO campbooks_app;
