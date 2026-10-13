-- The `conversations` module: inboxes, conversations, messages, labels, the
-- decide pass's answers and the member's corrections. Other modules' records
-- (users, connections) are referenced by id only.
--
-- Every table is workspace-owned and isolated by row-level security
-- (ADR 0011): the policy admits only the rows of the workspace set in the
-- transaction (`campbooks.workspace_id`, set by `begin_for` and by jobs).
-- `nullif(…, '')` because a setting a connection used before reads as ''
-- once its transaction ended; with nothing set, a query sees no row.
--
-- Message bodies, attachments and the participants document live in blob
-- storage (`kernel.blobs`); rows keep their `BlobRef`s as JSON.
CREATE SCHEMA IF NOT EXISTS conversations;

-- One address the workspace receives and sends on, on one channel. Created by
-- the connectors module when a connection grants the `messages` capability.
CREATE TABLE conversations.inboxes (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    channel text NOT NULL CHECK (channel IN ('email')),
    -- Lower-cased.
    address text NOT NULL CHECK (address = lower(address) AND char_length(address) BETWEEN 3 AND 320),
    display_name text,
    -- The connection it syncs through (an id owned by the connectors module), if any.
    connection_id uuid,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, channel, address)
);

-- Who sees an inbox. A user's conversations are those of their inboxes.
CREATE TABLE conversations.inbox_members (
    workspace_id uuid NOT NULL,
    inbox_id uuid NOT NULL REFERENCES conversations.inboxes (id) ON DELETE CASCADE,
    -- A user id (owned by the workspaces module).
    user_id uuid NOT NULL,
    role text NOT NULL CHECK (role IN ('reader', 'sender', 'manager')),
    PRIMARY KEY (inbox_id, user_id)
);
CREATE INDEX inbox_members_user_idx ON conversations.inbox_members (workspace_id, user_id);

CREATE TABLE conversations.conversations (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    inbox_id uuid NOT NULL REFERENCES conversations.inboxes (id) ON DELETE CASCADE,
    -- The provider's own thread id (Gmail `threadId`, Graph `conversationId`).
    thread_key text,
    -- The first message's subject.
    subject text NOT NULL DEFAULT '',
    -- Built-in streams; the workspace's own streams arrive with their table.
    stream text NOT NULL CHECK (stream IN ('primary', 'updates', 'receipts', 'newsletters')),
    -- Who put it there: nobody (`default`), an accepted rules answer, an
    -- accepted model answer, or a member.
    stream_source text NOT NULL CHECK (stream_source IN ('default', 'rules', 'model', 'member')),
    last_message_at timestamptz NOT NULL,
    message_count integer NOT NULL DEFAULT 0,
    unread_count integer NOT NULL DEFAULT 0,
    starred boolean NOT NULL DEFAULT false,
    -- Set while no message is in the inbox (archived here or at the provider).
    archived_at timestamptz,
    -- Every message is in the trash or spam.
    trashed boolean NOT NULL DEFAULT false,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX conversations_thread_key_idx ON conversations.conversations (inbox_id, thread_key)
    WHERE thread_key IS NOT NULL;
-- The inbox lists: by stream, newest first, keyset-paginated.
CREATE INDEX conversations_stream_list_idx
    ON conversations.conversations (inbox_id, stream, last_message_at DESC, id DESC)
    WHERE archived_at IS NULL AND NOT trashed;
CREATE INDEX conversations_list_idx
    ON conversations.conversations (inbox_id, last_message_at DESC, id DESC)
    WHERE NOT trashed;

CREATE TABLE conversations.messages (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    inbox_id uuid NOT NULL REFERENCES conversations.inboxes (id) ON DELETE CASCADE,
    conversation_id uuid NOT NULL REFERENCES conversations.conversations (id) ON DELETE CASCADE,
    provider_message_id text NOT NULL,
    direction text NOT NULL CHECK (direction IN ('inbound', 'outbound')),
    -- RFC 5322 `Message-ID`, `In-Reply-To` and `References`, for threading.
    rfc_message_id text,
    in_reply_to text,
    reference_ids text[] NOT NULL DEFAULT '{}',
    -- `{name, address}`, and its lower-cased address for filtering.
    sender jsonb NOT NULL,
    sender_address text NOT NULL,
    -- `{to: [...], cc: [...], reply_to: [...]}` of `{name, address}`.
    recipients jsonb NOT NULL,
    subject text NOT NULL DEFAULT '',
    -- The first characters of the text part, for list rows.
    snippet text NOT NULL DEFAULT '',
    sent_at timestamptz NOT NULL,
    -- The channel-specific headers subset (email: list and auto-submitted headers).
    headers jsonb NOT NULL DEFAULT '{}',
    -- The provider's normalised categories (`updates`, `promotions`, …).
    categories text[] NOT NULL DEFAULT '{}',
    -- The provider's labels on this message (their keys).
    provider_labels text[] NOT NULL DEFAULT '{}',
    in_inbox boolean NOT NULL,
    in_spam boolean NOT NULL DEFAULT false,
    in_trash boolean NOT NULL DEFAULT false,
    read boolean NOT NULL DEFAULT false,
    starred boolean NOT NULL DEFAULT false,
    -- `{raw?, text?, html?}` blob refs.
    bodies jsonb NOT NULL DEFAULT '{}',
    -- Blob ref of the participants document (addresses and subject).
    participants jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (inbox_id, provider_message_id)
);
CREATE INDEX messages_conversation_idx ON conversations.messages (conversation_id, sent_at);
-- Threading: a reply finds its ancestors by `Message-ID`, a late parent its
-- children by `In-Reply-To` / `References`.
CREATE INDEX messages_rfc_message_id_idx ON conversations.messages (inbox_id, rfc_message_id)
    WHERE rfc_message_id IS NOT NULL;
CREATE INDEX messages_in_reply_to_idx ON conversations.messages (inbox_id, in_reply_to)
    WHERE in_reply_to IS NOT NULL;
CREATE INDEX messages_reference_ids_idx ON conversations.messages USING gin (reference_ids);

CREATE TABLE conversations.attachments (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    message_id uuid NOT NULL REFERENCES conversations.messages (id) ON DELETE CASCADE,
    position integer NOT NULL,
    filename text NOT NULL,
    content_type text NOT NULL,
    size bigint NOT NULL CHECK (size >= 0),
    inline boolean NOT NULL DEFAULT false,
    blob jsonb NOT NULL
);
CREATE INDEX attachments_message_idx ON conversations.attachments (message_id, position);

-- A member's mark on a conversation, per inbox; mirrored with the provider
-- when it has labels (`provider_key`).
CREATE TABLE conversations.labels (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    inbox_id uuid NOT NULL REFERENCES conversations.inboxes (id) ON DELETE CASCADE,
    name text NOT NULL CHECK (char_length(name) BETWEEN 1 AND 100),
    provider_key text,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (inbox_id, name)
);
CREATE UNIQUE INDEX labels_provider_key_idx ON conversations.labels (inbox_id, provider_key)
    WHERE provider_key IS NOT NULL;

CREATE TABLE conversations.conversation_labels (
    workspace_id uuid NOT NULL,
    conversation_id uuid NOT NULL REFERENCES conversations.conversations (id) ON DELETE CASCADE,
    label_id uuid NOT NULL REFERENCES conversations.labels (id) ON DELETE CASCADE,
    PRIMARY KEY (conversation_id, label_id)
);
CREATE INDEX conversation_labels_label_idx ON conversations.conversation_labels (label_id);

-- Every answer the decide pass gave about a message, from rules or a model,
-- whether or not it was acted on.
CREATE TABLE conversations.decisions (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    message_id uuid NOT NULL REFERENCES conversations.messages (id) ON DELETE CASCADE,
    conversation_id uuid NOT NULL REFERENCES conversations.conversations (id) ON DELETE CASCADE,
    question text NOT NULL,
    source text NOT NULL CHECK (source IN ('rules', 'model', 'knn', 'cascade')),
    -- The strategy's own name (`rules`, `laya`, …).
    strategy text NOT NULL,
    -- A label (choice), a number (score) or a boolean (noul).
    value jsonb NOT NULL,
    -- The options in order, and their probabilities when the source knows them.
    options jsonb NOT NULL DEFAULT '[]',
    probabilities jsonb,
    confidence double precision,
    model text NOT NULL,
    model_version text,
    latency_ms integer NOT NULL DEFAULT 0,
    -- Why, in words a member can read ("List-Unsubscribe header").
    explanation text,
    -- Whether the answer changed anything (the per-question switch allowed it).
    acted boolean NOT NULL,
    -- The version of the envelope the answer was given about.
    state_version smallint NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (message_id, question, source)
);
CREATE INDEX decisions_conversation_idx ON conversations.decisions (conversation_id, question);

-- A member overriding where Campbooks put a conversation: a labelled example
-- for the question's eval set.
CREATE TABLE conversations.corrections (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    conversation_id uuid NOT NULL REFERENCES conversations.conversations (id) ON DELETE CASCADE,
    -- The latest inbound message when it was corrected.
    message_id uuid REFERENCES conversations.messages (id) ON DELETE SET NULL,
    question text NOT NULL,
    from_value text NOT NULL,
    to_value text NOT NULL,
    -- The `stream_source` it overrode.
    overridden_source text NOT NULL,
    -- The answers on record for that question at the time.
    decision_ids uuid[] NOT NULL DEFAULT '{}',
    -- A user id (workspaces module); NULL for a client token.
    corrected_by uuid,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX corrections_question_idx ON conversations.corrections (workspace_id, question, created_at);

-- Row-level security on every table. FORCE applies it to the tables' owner
-- too, for deployments whose runtime login owns them.
DO $$
DECLARE
    t text;
BEGIN
    FOREACH t IN ARRAY ARRAY['inboxes', 'inbox_members', 'conversations', 'messages',
                             'attachments', 'labels', 'conversation_labels', 'decisions',
                             'corrections']
    LOOP
        EXECUTE format('ALTER TABLE conversations.%I ENABLE ROW LEVEL SECURITY', t);
        EXECUTE format('ALTER TABLE conversations.%I FORCE ROW LEVEL SECURITY', t);
        EXECUTE format(
            'CREATE POLICY workspace_isolation ON conversations.%I USING '
            '(workspace_id = nullif(current_setting(''campbooks.workspace_id'', true), '''')::uuid)',
            t);
    END LOOP;
END
$$;

-- The runtime role (see `kernel__runtime_role`).
GRANT USAGE ON SCHEMA conversations TO campbooks_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA conversations TO campbooks_app;
