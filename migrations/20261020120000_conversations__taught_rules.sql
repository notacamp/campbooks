-- Taught rules (campbooks-cloud#125): deterministic condition → action pairs
-- a member taught Campbooks for one inbox, the runs that apply them to
-- existing mail, the conversations they changed, and the suggestions a member
-- turned down.

-- A rule may place a conversation and answer the decide pass's questions.
ALTER TABLE conversations.conversations DROP CONSTRAINT conversations_stream_source_check;
ALTER TABLE conversations.conversations ADD CONSTRAINT conversations_stream_source_check
    CHECK (stream_source IN ('default', 'rules', 'model', 'taught', 'member'));
ALTER TABLE conversations.decisions DROP CONSTRAINT decisions_source_check;
ALTER TABLE conversations.decisions ADD CONSTRAINT decisions_source_check
    CHECK (source IN ('rules', 'model', 'knn', 'cascade', 'taught'));
ALTER TABLE conversations.needs DROP CONSTRAINT needs_basis_check;
ALTER TABLE conversations.needs ADD CONSTRAINT needs_basis_check
    CHECK (basis IN ('rules', 'model', 'taught', 'member'));
ALTER TABLE conversations.needs DROP CONSTRAINT needs_cleared_because_check;
ALTER TABLE conversations.needs ADD CONSTRAINT needs_cleared_because_check
    CHECK (cleared_because IN
        ('replied', 'archived', 'trashed', 'done', 'not_needed', 'superseded', 'rule'));

-- The subject with what changes from one message to the next taken out
-- (`campbooks_ai::decide::signals::email::subject_template`); NULL when too
-- little is left, and for messages stored before this migration until the
-- backfill reaches them.
ALTER TABLE conversations.messages ADD COLUMN subject_template text;
CREATE INDEX messages_sender_idx ON conversations.messages (inbox_id, sender_address)
    WHERE direction = 'inbound';
CREATE INDEX messages_list_id_idx ON conversations.messages (inbox_id, (headers ->> 'list_id'))
    WHERE direction = 'inbound' AND headers ? 'list_id';
CREATE INDEX messages_subject_template_idx ON conversations.messages (inbox_id, subject_template)
    WHERE direction = 'inbound' AND subject_template IS NOT NULL;

CREATE TABLE conversations.rules (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    inbox_id uuid NOT NULL REFERENCES conversations.inboxes (id) ON DELETE CASCADE,
    -- Rules made by one teaching across several inboxes share it.
    teaching_id uuid NOT NULL,
    -- `[{field, value}]` (`campbooks_ai::decide::taught::Condition`), all must hold.
    conditions jsonb NOT NULL,
    -- `[{action, …}]` (`campbooks_ai::decide::taught::Action`).
    actions jsonb NOT NULL,
    -- The sum of the conditions' weights: the more specific rule wins.
    specificity integer NOT NULL,
    origin text NOT NULL
        CHECK (origin IN ('action', 'onboarding', 'instruction', 'correction', 'suggestion')),
    -- The member's own words, for an instruction. Personal data.
    origin_text text,
    state text NOT NULL CHECK (state IN ('proposed', 'active', 'paused', 'removed')),
    -- A user id (workspaces module): who taught it, and whose rights it acts with.
    taught_by uuid,
    -- The conversation it was taught from, for an action.
    from_conversation_id uuid,
    created_at timestamptz NOT NULL DEFAULT now(),
    confirmed_at timestamptz,
    removed_at timestamptz,
    applied_count integer NOT NULL DEFAULT 0,
    last_applied_at timestamptz,
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX rules_inbox_idx ON conversations.rules (inbox_id, state);

-- One pass of a rule over existing mail: applying it, or putting back what
-- it changed. Resumable from `cursor` (the last conversation id done).
CREATE TABLE conversations.rule_runs (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    rule_id uuid NOT NULL REFERENCES conversations.rules (id) ON DELETE CASCADE,
    kind text NOT NULL CHECK (kind IN ('apply', 'revert')),
    state text NOT NULL CHECK (state IN ('running', 'done', 'cancelled')),
    cursor uuid,
    total integer NOT NULL DEFAULT 0,
    done integer NOT NULL DEFAULT 0,
    -- Changed by the run (the rest were already so, or another rule wins).
    changed integer NOT NULL DEFAULT 0,
    -- Left alone: they look suspicious, or a member placed or changed them.
    skipped_suspicious integer NOT NULL DEFAULT 0,
    skipped_member integer NOT NULL DEFAULT 0,
    started_at timestamptz NOT NULL DEFAULT now(),
    finished_at timestamptz
);
CREATE INDEX rule_runs_rule_idx ON conversations.rule_runs (rule_id, started_at DESC);

-- A conversation a rule changed, and what it was before, so the change can be
-- put back. A member changing it back makes it an exception the rule leaves alone.
CREATE TABLE conversations.rule_applications (
    rule_id uuid NOT NULL REFERENCES conversations.rules (id) ON DELETE CASCADE,
    conversation_id uuid NOT NULL REFERENCES conversations.conversations (id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL,
    -- The run that changed it; NULL when it was changed as mail arrived.
    run_id uuid REFERENCES conversations.rule_runs (id) ON DELETE SET NULL,
    -- `{stream, stream_source, archived, read, labels}` before the change.
    before jsonb NOT NULL,
    applied_at timestamptz NOT NULL DEFAULT now(),
    reverted_at timestamptz,
    excepted_at timestamptz,
    excepted_by uuid,
    PRIMARY KEY (rule_id, conversation_id)
);
CREATE INDEX rule_applications_conversation_idx
    ON conversations.rule_applications (conversation_id);

-- A suggestion a member turned down: hidden for 30 days, for good after two.
CREATE TABLE conversations.rule_dismissals (
    workspace_id uuid NOT NULL,
    inbox_id uuid NOT NULL REFERENCES conversations.inboxes (id) ON DELETE CASCADE,
    user_id uuid NOT NULL,
    -- The likeness and the action, as one stable key (no personal data in it
    -- beyond what the rule tables already hold).
    suggestion_key text NOT NULL,
    times integer NOT NULL DEFAULT 1,
    dismissed_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (inbox_id, user_id, suggestion_key)
);

DO $$
DECLARE
    t text;
BEGIN
    FOREACH t IN ARRAY ARRAY['rules', 'rule_runs', 'rule_applications', 'rule_dismissals']
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
    ON conversations.rules, conversations.rule_runs, conversations.rule_applications,
       conversations.rule_dismissals
    TO campbooks_app;
