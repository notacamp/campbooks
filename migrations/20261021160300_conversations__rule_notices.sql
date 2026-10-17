-- "Scout noticed" (campbooks-cloud#169): smart rules Scout offers for mail an
-- inbox gets a lot of, the members' answers to them, and when each inbox was
-- last looked at. Every row carries its workspace (forced row-level
-- security) and its inbox, so the inbox's purge takes them with it. Only new
-- objects are created; the timeout keeps an unexpected lock wait from
-- holding up a deploy.
SET LOCAL lock_timeout = '5s';

-- One suggestion per inbox, kind and sender: refreshed by each pass while
-- the sender still qualifies (`open`), kept once a member said yes to it
-- (`accepted`, naming the rule it became). Addresses and counts only, never
-- message text.
CREATE TABLE conversations.rule_notices (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    inbox_id uuid NOT NULL REFERENCES conversations.inboxes (id) ON DELETE CASCADE,
    kind text NOT NULL CHECK (kind IN ('keep_off_home', 'archive_on_arrival')),
    sender_address text NOT NULL CHECK (char_length(sender_address) <= 320),
    -- The name the sender gives, shown as plain text.
    sender_name text CHECK (char_length(sender_name) <= 200),
    conditions jsonb NOT NULL,
    actions jsonb NOT NULL,
    -- The counts behind it (received, opened, archived unread, cadence).
    evidence jsonb NOT NULL,
    state text NOT NULL DEFAULT 'open' CHECK (state IN ('open', 'accepted')),
    -- The rule it became (no foreign key: a removed rule stays a row).
    rule_id uuid,
    accepted_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    refreshed_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (inbox_id, kind, sender_address)
);
CREATE INDEX rule_notices_rule_idx ON conversations.rule_notices (rule_id)
    WHERE rule_id IS NOT NULL;

-- A member's "Not now" (until a time) or "Don't suggest this" (for good), per
-- sender and kind, theirs alone.
CREATE TABLE conversations.rule_notice_answers (
    workspace_id uuid NOT NULL,
    inbox_id uuid NOT NULL REFERENCES conversations.inboxes (id) ON DELETE CASCADE,
    -- A user id (workspaces module).
    user_id uuid NOT NULL,
    kind text NOT NULL CHECK (kind IN ('keep_off_home', 'archive_on_arrival')),
    sender_address text NOT NULL CHECK (char_length(sender_address) <= 320),
    answer text NOT NULL CHECK (answer IN ('not_now', 'never')),
    until timestamptz,
    answered_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (inbox_id, user_id, kind, sender_address),
    CHECK ((answer = 'not_now') = (until IS NOT NULL))
);

-- When each inbox was last looked at, so a pass runs at most so often.
CREATE TABLE conversations.rule_notice_scans (
    inbox_id uuid PRIMARY KEY REFERENCES conversations.inboxes (id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL,
    scanned_at timestamptz NOT NULL
);

DO $$
DECLARE
    t text;
BEGIN
    FOREACH t IN ARRAY ARRAY['rule_notices', 'rule_notice_answers', 'rule_notice_scans']
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

GRANT SELECT, INSERT, UPDATE, DELETE
    ON conversations.rule_notices, conversations.rule_notice_answers,
       conversations.rule_notice_scans
    TO campbooks_app;
