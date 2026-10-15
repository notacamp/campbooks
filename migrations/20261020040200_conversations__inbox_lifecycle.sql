SET LOCAL lock_timeout = '5s';

-- An inbox's owner, its read-only state, and the inboxes that were deleted
-- (`openspec/changes/access-guards`, ADR 0017).
--
-- `owner`: the member whose connection feeds it (the access catalogue's
-- owner; a user id of the workspaces module). Cleared when they leave the
-- workspace, so coming back never brings back what they had.
-- `detached_at`: set when its connection was removed and the mail kept as
-- read-only history: nothing syncs into it, nothing is sent from it.

-- Existing inboxes keep no owner (NULL): who connected them is not known
-- here for certain, and an owner is never guessed. New ones get it from the
-- grant.
ALTER TABLE conversations.inboxes
    ADD COLUMN owner uuid,
    ADD COLUMN detached_at timestamptz;

-- When a membership began: a member's departure removes only the ones
-- older than it, never one given after they came back.
ALTER TABLE conversations.inbox_members
    ADD COLUMN created_at timestamptz NOT NULL DEFAULT now();

-- Inboxes deleted with their connection: a late batch or a replayed grant
-- never brings one back. Ids only.
CREATE TABLE conversations.removed_inboxes (
    inbox_id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    removed_at timestamptz NOT NULL DEFAULT now()
);

ALTER TABLE conversations.removed_inboxes ENABLE ROW LEVEL SECURITY;
ALTER TABLE conversations.removed_inboxes FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON conversations.removed_inboxes USING
    (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);

GRANT SELECT, INSERT, UPDATE, DELETE ON conversations.removed_inboxes TO campbooks_app;
