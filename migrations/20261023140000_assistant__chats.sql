-- Scout's chats (campbooks-cloud#305): a member asks Scout about something
-- they are looking at, and Scout answers from how Campbooks works and from
-- what it reads as that member. Private to the member, kept 30 days after
-- the last message, deleted when the member leaves or the workspace goes.
-- Other modules' records are referenced by type and id only (no foreign
-- key): `about_type` is a record type of the chat's about registry, checked
-- in code, so a new type needs no migration.
-- Only new objects are created here; the timeout keeps an unexpected lock
-- wait from holding up a deploy.
SET LOCAL lock_timeout = '5s';

CREATE TABLE assistant.chats (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    -- A user id (workspaces module): the member whose chat it is.
    user_id uuid NOT NULL,
    -- What it is about: a record type (`conversations.conversation`) and
    -- the record's id, or neither.
    about_type text CHECK (char_length(about_type) BETWEEN 3 AND 64),
    about_id text CHECK (char_length(about_id) BETWEEN 1 AND 128),
    -- Questions asked so far (at most 40, checked by the module).
    questions integer NOT NULL DEFAULT 0 CHECK (questions >= 0),
    created_at timestamptz NOT NULL DEFAULT now(),
    last_message_at timestamptz NOT NULL DEFAULT now(),
    CHECK ((about_type IS NULL) = (about_id IS NULL))
);
-- One chat per member and record: starting again continues it.
CREATE UNIQUE INDEX chats_about_key
    ON assistant.chats (workspace_id, user_id, about_type, about_id)
    WHERE about_id IS NOT NULL;
CREATE INDEX chats_member_idx ON assistant.chats (workspace_id, user_id);
-- What the daily expiry looks for.
CREATE INDEX chats_last_message_idx ON assistant.chats (last_message_at);

CREATE TABLE assistant.chat_messages (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    chat_id uuid NOT NULL REFERENCES assistant.chats (id) ON DELETE CASCADE,
    author text NOT NULL CHECK (author IN ('member', 'scout')),
    -- What the member asked, or Scout's answer. Personal data.
    text text NOT NULL,
    -- Scout's steps: `[{tool, records: [{type, id}], count}]`, built by the
    -- server from the tools that ran, never from the model's words.
    steps jsonb NOT NULL DEFAULT '[]',
    -- How Scout's answer ended; NULL for the member's question.
    finish text CHECK (finish IN ('done', 'failed', 'cut_short')),
    created_at timestamptz NOT NULL DEFAULT now(),
    CHECK ((author = 'scout') = (finish IS NOT NULL))
);
CREATE INDEX chat_messages_chat_idx ON assistant.chat_messages (chat_id, created_at);

ALTER TABLE assistant.chats ENABLE ROW LEVEL SECURITY;
ALTER TABLE assistant.chats FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON assistant.chats
    USING (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);
-- The daily expiry's dispatcher (`assistant.dispatch_expire_chats`) reads,
-- in the maintenance scope, which workspaces hold a chat to expire: read
-- only, and it reads nothing but ids and times.
CREATE POLICY maintenance_reads_chats ON assistant.chats FOR SELECT
    USING (current_setting('campbooks.maintenance_scope', true) = 'on');

ALTER TABLE assistant.chat_messages ENABLE ROW LEVEL SECURITY;
ALTER TABLE assistant.chat_messages FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON assistant.chat_messages
    USING (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);

GRANT SELECT, INSERT, UPDATE, DELETE ON assistant.chats, assistant.chat_messages
    TO campbooks_app;
