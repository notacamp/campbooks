-- An inbox's owner, its read-only state, and the inboxes that were deleted
-- (`openspec/changes/access-guards`, ADR 0017).
--
-- `owner`: the member whose connection feeds it (the access catalogue's
-- owner; a user id of the workspaces module). Cleared when they leave the
-- workspace, so coming back never brings back what they had.
-- `detached_at`: set when its connection was removed and the mail kept as
-- read-only history: nothing syncs into it, nothing is sent from it.
ALTER TABLE conversations.inboxes
    ADD COLUMN owner uuid,
    ADD COLUMN detached_at timestamptz;

-- Existing inboxes: the manager who created them (the granting member).
UPDATE conversations.inboxes i
   SET owner = (SELECT m.user_id FROM conversations.inbox_members m
                 WHERE m.inbox_id = i.id AND m.role = 'manager'
                 ORDER BY m.user_id LIMIT 1);

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
