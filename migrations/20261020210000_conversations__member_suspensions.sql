-- A suspended member's rules pause (campbooks-cloud#147).
--
-- Whether each member of the workspace is suspended, as the
-- `workspaces.member_suspended` / `workspaces.member_unsuspended` events
-- said. One row per member who was ever suspended here; a lift keeps the row
-- (`suspended = false`) so a suspension delivered late, with an older
-- `as_of`, changes nothing. A rule whose author is suspended reads as paused.
-- Ids and times only; deleted with the workspace.
CREATE TABLE conversations.member_suspensions (
    workspace_id uuid NOT NULL,
    user_id uuid NOT NULL,
    suspended boolean NOT NULL,
    -- When the newest change recorded took effect (the event's `at`).
    as_of timestamptz NOT NULL,
    PRIMARY KEY (workspace_id, user_id)
);

ALTER TABLE conversations.member_suspensions ENABLE ROW LEVEL SECURITY;
ALTER TABLE conversations.member_suspensions FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON conversations.member_suspensions
    USING (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);

GRANT SELECT, INSERT, UPDATE, DELETE ON conversations.member_suspensions TO campbooks_app;
