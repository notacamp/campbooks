-- The `tasks` module owns the `tasks` schema and nothing else: what a member
-- has to do (campbooks-cloud#43). Other modules' records are referenced by
-- type and id only (no cross-schema joins or foreign keys).
CREATE SCHEMA IF NOT EXISTS tasks;

CREATE TABLE tasks.tasks (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    title text NOT NULL CHECK (char_length(title) BETWEEN 1 AND 200),
    verb text NOT NULL CHECK (verb IN ('do', 'pay')),
    origin text NOT NULL CHECK (origin IN ('own', 'ask', 'deadline')),
    due_on date,
    -- With a time: `due_on` is its local day.
    due_at timestamptz,
    plan text NOT NULL DEFAULT 'none' CHECK (plan IN ('none', 'date', 'later')),
    plan_on date,
    later_until timestamptz,
    state text NOT NULL DEFAULT 'open' CHECK (state IN ('open', 'done', 'not_needed')),
    closed_at timestamptz,
    -- User ids (workspaces module).
    assignee uuid NOT NULL,
    created_by uuid NOT NULL,
    handed_by uuid,
    -- The record it came from (`conversations.conversation`), the inbox it
    -- lives in and the sender's address, kept for `OpenTasks`; never in an event.
    source_type text,
    source_id text,
    source_container text,
    source_sender text,
    amount_minor bigint,
    currency text,
    revision bigint NOT NULL DEFAULT 1,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK ((source_type IS NULL) = (source_id IS NULL)),
    CHECK (plan <> 'date' OR plan_on IS NOT NULL),
    CHECK (plan <> 'later' OR later_until IS NOT NULL),
    CHECK (due_at IS NULL OR due_on IS NOT NULL)
);
CREATE INDEX tasks_assignee_idx ON tasks.tasks (workspace_id, assignee, state, due_on);
CREATE INDEX tasks_handed_by_idx ON tasks.tasks (workspace_id, handed_by) WHERE handed_by IS NOT NULL;
CREATE INDEX tasks_source_idx ON tasks.tasks (workspace_id, source_id) WHERE state = 'open';
CREATE INDEX tasks_sender_idx ON tasks.tasks (workspace_id, source_container, source_sender)
    WHERE state = 'open' AND source_sender IS NOT NULL;

CREATE TABLE tasks.history (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    task_id uuid NOT NULL REFERENCES tasks.tasks (id) ON DELETE CASCADE,
    at timestamptz NOT NULL DEFAULT now(),
    actor text NOT NULL CHECK (actor IN ('member', 'scout', 'calendar')),
    actor_member uuid,
    what text NOT NULL CHECK (what IN ('created', 'edited', 'due_changed', 'planned', 'done',
                                       'not_needed', 'reopened', 'handed_off', 'taken_back')),
    -- Ids, enums and dates only.
    detail jsonb NOT NULL DEFAULT '{}'
);
CREATE INDEX history_task_idx ON tasks.history (task_id, at);

DO $$
DECLARE
    t text;
BEGIN
    FOREACH t IN ARRAY ARRAY['tasks', 'history']
    LOOP
        EXECUTE format('ALTER TABLE tasks.%I ENABLE ROW LEVEL SECURITY', t);
        EXECUTE format('ALTER TABLE tasks.%I FORCE ROW LEVEL SECURITY', t);
        EXECUTE format(
            'CREATE POLICY workspace_isolation ON tasks.%I USING '
            '(workspace_id = nullif(current_setting(''campbooks.workspace_id'', true), '''')::uuid)',
            t);
    END LOOP;
END
$$;

GRANT USAGE ON SCHEMA tasks TO campbooks_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA tasks TO campbooks_app;
