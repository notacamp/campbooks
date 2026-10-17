-- The changes a member asked the provider to mirror, as this module asked
-- for them (campbooks-cloud#226).
--
-- Every `conversations.changes_requested` is recorded here under the event's
-- id, in the transaction that publishes it. The connector asks this module,
-- through the kernel's outbound-requests port, whether a change request it
-- received was really made here, and makes only the changes recorded with
-- it: any transaction working in the workspace can append the event. Ids
-- and provider message ids only, no content. Rows go with their
-- conversation, and older than seven days with the next request on it.
SET LOCAL lock_timeout = '10s';

CREATE TABLE conversations.change_requests (
    -- The id of the `conversations.changes_requested` event.
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    inbox_id uuid NOT NULL,
    conversation_id uuid NOT NULL
        REFERENCES conversations.conversations (id) ON DELETE CASCADE,
    -- The event's items: `[{provider_message_id, change}]`.
    items jsonb NOT NULL CHECK (jsonb_typeof(items) = 'array'),
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX change_requests_conversation_idx
    ON conversations.change_requests (conversation_id, created_at);

ALTER TABLE conversations.change_requests ENABLE ROW LEVEL SECURITY;
ALTER TABLE conversations.change_requests FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON conversations.change_requests USING
    (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);

GRANT SELECT, INSERT, DELETE ON conversations.change_requests TO campbooks_app;
