-- A member's signature per inbox they send from (campbooks-cloud#393), and
-- the signature a draft carries.
--
-- A signature is its member's own: one row per inbox and member, private to
-- that member (every statement is keyed by the caller's own id). It holds
-- text with the marks a message may carry, at most 2000 characters. A draft
-- copies the author's signature when it is created, so editing a signature
-- never changes a message already started; `drafts.signature` is '' for a
-- draft that carries none. Adding a column with a constant default is
-- catalog-only on Postgres 11+. The member's departure (`forget_member`)
-- and the workspace's purge delete the rows. Only a new table and a column
-- are added; the timeout keeps an unexpected lock wait from holding up a
-- deploy.
SET LOCAL lock_timeout = '5s';

CREATE TABLE conversations.signatures (
    workspace_id uuid NOT NULL,
    inbox_id uuid NOT NULL REFERENCES conversations.inboxes (id) ON DELETE CASCADE,
    user_id uuid NOT NULL,
    text text NOT NULL CHECK (char_length(text) <= 2000),
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (inbox_id, user_id)
);

-- A member's departure and the workspace's purge delete by these.
CREATE INDEX signatures_member_idx ON conversations.signatures (workspace_id, user_id);

ALTER TABLE conversations.signatures ENABLE ROW LEVEL SECURITY;
ALTER TABLE conversations.signatures FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON conversations.signatures USING
    (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);

GRANT SELECT, INSERT, UPDATE, DELETE ON conversations.signatures TO campbooks_app;

ALTER TABLE conversations.drafts ADD COLUMN signature text NOT NULL DEFAULT '';
