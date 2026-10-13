-- The `attention` module owns the `attention` schema and nothing else: one
-- queue of what needs each member (campbooks-cloud#39). Other modules'
-- records are referenced by type and id only (no cross-schema joins or
-- foreign keys), and no row holds a name, an address, a subject or text.
CREATE SCHEMA IF NOT EXISTS attention;

-- What needs the members who can see `container`: a projection of the owning
-- modules' events, rebuilt by replaying them.
CREATE TABLE attention.items (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    -- The record it is about (`conversations.conversation`).
    record_type text NOT NULL,
    record_id text NOT NULL,
    -- Who may see it: members the container's module admits (`conversations.inbox`).
    container_type text NOT NULL,
    container_id text NOT NULL,
    -- The record that raised it (`conversations.message`): a new cause ends a snooze.
    cause_type text NOT NULL,
    cause_id text NOT NULL,
    reason text NOT NULL CHECK (reason IN ('review', 'reply', 'decide', 'do', 'pay')),
    trust text NOT NULL CHECK (trust IN ('sure', 'check')),
    due_on date,
    urgent boolean NOT NULL DEFAULT false,
    -- When its cause arrived.
    since timestamptz NOT NULL,
    state text NOT NULL CHECK (state IN ('open', 'cleared')),
    cleared_because text,
    -- A user id (workspaces module) when a member cleared it.
    cleared_by uuid,
    cleared_at timestamptz,
    -- The producer's revision of this record's need: only newer ones apply.
    producer_revision bigint NOT NULL,
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, record_type, record_id)
);
CREATE INDEX items_open_idx ON attention.items (workspace_id, reason, due_on) WHERE state = 'open';
CREATE INDEX items_cleared_idx ON attention.items (workspace_id, cleared_at DESC)
    WHERE state = 'cleared';

-- "Later": a member's own snooze of an item, until a time or a new cause.
CREATE TABLE attention.snoozes (
    workspace_id uuid NOT NULL,
    item_id uuid NOT NULL REFERENCES attention.items (id) ON DELETE CASCADE,
    -- A user id (workspaces module).
    user_id uuid NOT NULL,
    until timestamptz NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (item_id, user_id)
);
CREATE INDEX snoozes_user_idx ON attention.snoozes (workspace_id, user_id);

DO $$
DECLARE
    t text;
BEGIN
    FOREACH t IN ARRAY ARRAY['items', 'snoozes']
    LOOP
        EXECUTE format('ALTER TABLE attention.%I ENABLE ROW LEVEL SECURITY', t);
        EXECUTE format('ALTER TABLE attention.%I FORCE ROW LEVEL SECURITY', t);
        EXECUTE format(
            'CREATE POLICY workspace_isolation ON attention.%I USING '
            '(workspace_id = nullif(current_setting(''campbooks.workspace_id'', true), '''')::uuid)',
            t);
    END LOOP;
END
$$;

GRANT USAGE ON SCHEMA attention TO campbooks_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA attention TO campbooks_app;
