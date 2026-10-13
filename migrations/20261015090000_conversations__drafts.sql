-- Drafts: a member's message being written in one inbox, private to its
-- author, and its send (undo window, release, outcome). Users and the
-- connector's records are referenced by id only.
CREATE TABLE conversations.drafts (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    inbox_id uuid NOT NULL REFERENCES conversations.inboxes (id) ON DELETE CASCADE,
    -- A user id (workspaces module); NULL for a client token's draft.
    author uuid,
    kind text NOT NULL CHECK (kind IN ('new', 'reply', 'reply_all', 'forward')),
    conversation_id uuid REFERENCES conversations.conversations (id) ON DELETE SET NULL,
    -- The message replied to or forwarded.
    source_message_id uuid REFERENCES conversations.messages (id) ON DELETE SET NULL,
    -- `[{name?, address}]`.
    to_recipients jsonb NOT NULL DEFAULT '[]',
    cc_recipients jsonb NOT NULL DEFAULT '[]',
    bcc_recipients jsonb NOT NULL DEFAULT '[]',
    subject text NOT NULL DEFAULT '' CHECK (char_length(subject) <= 998),
    -- What the member writes, and the quoted history under it.
    body text NOT NULL DEFAULT '',
    quoted text NOT NULL DEFAULT '',
    -- Threading, filled for replies and forwards.
    in_reply_to text,
    reference_ids text[] NOT NULL DEFAULT '{}',
    thread_key text,
    -- Forwarded attachments: `[{id, filename, content_type, size, blob}]`,
    -- their blobs held by the draft.
    attachments jsonb NOT NULL DEFAULT '[]',
    -- Bumped by every save.
    version integer NOT NULL DEFAULT 1,
    state text NOT NULL DEFAULT 'draft' CHECK (state IN ('draft', 'scheduled', 'sending', 'failed')),
    -- Bumped by every send; a release job acts only at its generation.
    send_generation integer NOT NULL DEFAULT 0,
    send_at timestamptz,
    -- Minted when the send is released.
    send_id uuid UNIQUE,
    rfc_message_id text,
    failure_reason text,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX drafts_author_idx ON conversations.drafts (workspace_id, author, updated_at DESC);
CREATE INDEX drafts_conversation_idx ON conversations.drafts (conversation_id)
    WHERE conversation_id IS NOT NULL;

ALTER TABLE conversations.drafts ENABLE ROW LEVEL SECURITY;
ALTER TABLE conversations.drafts FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON conversations.drafts USING
    (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);

GRANT SELECT, INSERT, UPDATE, DELETE ON conversations.drafts TO campbooks_app;
