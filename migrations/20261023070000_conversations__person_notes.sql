-- What a member told Scout about a person (campbooks-cloud#272): who they
-- are and how to write to them, private to that member and used only when
-- Scout writes that member a reply to the person.
--
-- One note per member and address. It holds personal data: every row
-- carries its workspace (forced row-level security), the member's own id
-- (`user_id`, a user of the workspaces module, referenced by id only) is
-- checked by every statement that reads or changes a note, and the
-- workspace's purge and the member's departure (`forget_member`) delete
-- them. The unique key also serves listing a member's notes and deleting
-- them by member or by workspace. Only a new table is created; the timeout
-- keeps an unexpected lock wait from holding up a deploy.
SET LOCAL lock_timeout = '5s';

CREATE TABLE conversations.person_notes (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    user_id uuid NOT NULL,
    -- Lower-cased, as the mail's addresses are compared.
    address text NOT NULL
        CHECK (address = lower(address) AND char_length(address) BETWEEN 3 AND 320),
    note text NOT NULL CHECK (char_length(note) BETWEEN 1 AND 500),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, user_id, address)
);

ALTER TABLE conversations.person_notes ENABLE ROW LEVEL SECURITY;
ALTER TABLE conversations.person_notes FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON conversations.person_notes USING
    (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);

GRANT SELECT, INSERT, UPDATE, DELETE ON conversations.person_notes TO campbooks_app;
