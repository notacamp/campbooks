-- "Scout noticed" finds patterns within a frequent sender's mail
-- (campbooks-cloud#356): a suggestion is now for one kind of a sender's
-- mail, the whole sender (`pattern` '') or a part of it (`t:<template>`,
-- the subject template; `p:<words>`, words the `complete` model proposed and
-- the pass checked against the sender's mail), with one real subject of it
-- as an example. Answers are per kind too. The model's grouping of a
-- sender's subjects is kept for a few days, so it is asked rarely. Columns
-- are added with defaults, so every row already stored stays valid and means
-- what it meant (the whole sender); the timeout keeps an unexpected lock
-- wait from holding up a deploy.
SET LOCAL lock_timeout = '5s';

ALTER TABLE conversations.rule_notices
    ADD COLUMN pattern text NOT NULL DEFAULT '' CHECK (char_length(pattern) <= 400),
    -- One recent subject of the kind, shown to members who can read that
    -- mail anyway; none for the whole sender.
    ADD COLUMN example_subject text CHECK (char_length(example_subject) <= 200);
ALTER TABLE conversations.rule_notices
    DROP CONSTRAINT IF EXISTS rule_notices_inbox_id_kind_sender_address_key;
ALTER TABLE conversations.rule_notices
    ADD CONSTRAINT rule_notices_inbox_kind_sender_pattern_key
        UNIQUE (inbox_id, kind, sender_address, pattern);
COMMENT ON TABLE conversations.rule_notices IS
    'Smart rules Scout offers for mail an inbox gets a lot of: one per inbox, kind, sender and '
    'pattern. Addresses, counts and one example subject of the mail it is about, nothing else '
    'of the text.';

ALTER TABLE conversations.rule_notice_answers
    ADD COLUMN pattern text NOT NULL DEFAULT '' CHECK (char_length(pattern) <= 400);
ALTER TABLE conversations.rule_notice_answers
    DROP CONSTRAINT IF EXISTS rule_notice_answers_pkey;
ALTER TABLE conversations.rule_notice_answers
    ADD CONSTRAINT rule_notice_answers_pkey
        PRIMARY KEY (inbox_id, user_id, kind, sender_address, pattern);

-- The `complete` model's grouping of one sender's subjects: the phrases it
-- proposed, normalised (lower-cased, accents dropped), possibly none. Reused
-- until it is `regroup_days` old.
CREATE TABLE conversations.rule_notice_groupings (
    inbox_id uuid NOT NULL REFERENCES conversations.inboxes (id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL,
    sender_address text NOT NULL CHECK (char_length(sender_address) <= 320),
    phrases jsonb NOT NULL CHECK (jsonb_typeof(phrases) = 'array'),
    grouped_at timestamptz NOT NULL,
    PRIMARY KEY (inbox_id, sender_address)
);

ALTER TABLE conversations.rule_notice_groupings ENABLE ROW LEVEL SECURITY;
ALTER TABLE conversations.rule_notice_groupings FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON conversations.rule_notice_groupings USING
    (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);

GRANT SELECT, INSERT, UPDATE, DELETE ON conversations.rule_notice_groupings TO campbooks_app;
