-- Scout explains a thread (campbooks-cloud#407, OpenSpec change
-- explain-a-thread): a member asks, and Scout's explanation of one
-- conversation is kept for them alone until the thread changes.
--
-- One row per member and conversation. It holds what the model wrote about
-- a member's mail, so it is private to that member (every read and write is
-- keyed by `member_id`, checked again against the member's access on every
-- read), it goes with its conversation (the foreign key), with its member
-- (`forget_member`), when the member loses the inbox (the trigger below),
-- and 30 days after it was written (`conversations.expire_explanations`).
--
-- `asked_version` and `asked_locale` describe the latest request, which is
-- what a second click is coalesced with. `thread_version`, `locale`,
-- `message_ids`, `in_full` and `body` describe the explanation kept, so a
-- request that fails (the daily limit) leaves the earlier explanation as it
-- was. Idempotent: a second run changes nothing.
SET LOCAL lock_timeout = '5s';

CREATE TABLE IF NOT EXISTS conversations.explanations (
    workspace_id uuid NOT NULL,
    -- A user id (owned by the workspaces module).
    member_id uuid NOT NULL,
    conversation_id uuid NOT NULL
        REFERENCES conversations.conversations (id) ON DELETE CASCADE,
    state text NOT NULL CHECK (state IN ('writing', 'ready', 'failed')),
    failure text CHECK (failure IN ('limit', 'unusable', 'unavailable')),
    -- The newest message and the message count when the latest request was
    -- accepted, and the language it was asked in.
    asked_version text NOT NULL,
    asked_locale text NOT NULL CHECK (asked_locale IN ('en', 'pt-PT', 'es-ES', 'fr')),
    -- How the member is named in the prompt: their own name, kept only for
    -- the job that writes the explanation.
    reader text CHECK (char_length(reader) <= 200),
    -- The explanation kept: its language, the thread it was made from, every
    -- message whose words or read went in, how many of them in full.
    locale text CHECK (locale IN ('en', 'pt-PT', 'es-ES', 'fr')),
    thread_version text,
    message_ids uuid[] NOT NULL DEFAULT '{}',
    in_full integer NOT NULL DEFAULT 0 CHECK (in_full >= 0),
    body jsonb CHECK (jsonb_typeof(body) = 'object'),
    model text,
    written_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (workspace_id, member_id, conversation_id),
    CONSTRAINT explanations_failed_with_reason_check CHECK ((state = 'failed') = (failure IS NOT NULL)),
    CONSTRAINT explanations_ready_has_body_check CHECK (state <> 'ready' OR body IS NOT NULL)
);

-- The conversation's cascade, and removing a member's rows by inbox.
CREATE INDEX IF NOT EXISTS explanations_conversation_idx
    ON conversations.explanations (conversation_id);
-- The daily expiry looks for the old ones.
CREATE INDEX IF NOT EXISTS explanations_age_idx
    ON conversations.explanations ((COALESCE(written_at, updated_at)));

ALTER TABLE conversations.explanations ENABLE ROW LEVEL SECURITY;
ALTER TABLE conversations.explanations FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS workspace_isolation ON conversations.explanations;
CREATE POLICY workspace_isolation ON conversations.explanations USING
    (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);
-- The daily dispatcher reads which workspaces hold an old row (ids and times
-- only); it deletes nothing itself.
DROP POLICY IF EXISTS maintenance_reads_explanations ON conversations.explanations;
CREATE POLICY maintenance_reads_explanations ON conversations.explanations FOR SELECT
    USING (current_setting('campbooks.maintenance_scope', true) = 'on');

GRANT SELECT, INSERT, UPDATE, DELETE ON conversations.explanations TO campbooks_app;

-- A member who is taken off an inbox loses the explanations of its
-- conversations, whichever code removed them. Runs as the deleting
-- transaction, in its workspace.
CREATE OR REPLACE FUNCTION conversations.drop_explanations_of_removed_member()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    DELETE FROM conversations.explanations e
     USING conversations.conversations c
     WHERE c.id = e.conversation_id
       AND c.inbox_id = OLD.inbox_id
       AND e.member_id = OLD.user_id;
    RETURN OLD;
END
$$;

DROP TRIGGER IF EXISTS inbox_members_drop_explanations ON conversations.inbox_members;
CREATE TRIGGER inbox_members_drop_explanations
    AFTER DELETE ON conversations.inbox_members
    FOR EACH ROW EXECUTE FUNCTION conversations.drop_explanations_of_removed_member();
