-- AI usage per workspace and UTC day, and the daily cap (campbooks-cloud#152;
-- the `system` module's `AiUsageMeter`, behind `campbooks_ai::usage::Meter`).
-- Counts and ids only, never content.

-- The cap's counter: one row per workspace and day. A call is admitted by
-- one conditional upsert on this row (`calls < cap`), so concurrent workers
-- never admit more than the cap.
CREATE TABLE system.ai_usage_days (
    workspace_id uuid NOT NULL,
    day date NOT NULL,
    -- Calls admitted (every role and feature).
    calls integer NOT NULL DEFAULT 0 CHECK (calls >= 0),
    -- Calls refused over the cap.
    refused integer NOT NULL DEFAULT 0 CHECK (refused >= 0),
    -- The first refusal of the day.
    capped_at timestamptz,
    PRIMARY KEY (workspace_id, day)
);
CREATE INDEX ai_usage_days_day_idx ON system.ai_usage_days (day);

-- The ledger by role and feature (`conversations.triage`, …).
CREATE TABLE system.ai_usage (
    workspace_id uuid NOT NULL,
    day date NOT NULL,
    role text NOT NULL CHECK (role IN ('decide', 'embed', 'complete', 'vision')),
    feature text NOT NULL CHECK (feature ~ '^[a-z_]+\.[a-z_]+$'),
    calls integer NOT NULL DEFAULT 0 CHECK (calls >= 0),
    -- Admitted calls that failed (no answer, an unusable answer).
    failed integer NOT NULL DEFAULT 0 CHECK (failed >= 0),
    -- Tokens, as the providers reported them (0 when they did not).
    prompt_tokens bigint NOT NULL DEFAULT 0 CHECK (prompt_tokens >= 0),
    completion_tokens bigint NOT NULL DEFAULT 0 CHECK (completion_tokens >= 0),
    PRIMARY KEY (workspace_id, day, role, feature)
);

-- Each workspace sees its own rows; an administrator's transaction
-- (`kernel::db::begin_admin`, ADR 0015) reads every workspace's counts.
DO $$
DECLARE
    t text;
BEGIN
    FOREACH t IN ARRAY ARRAY['ai_usage_days', 'ai_usage']
    LOOP
        EXECUTE format('ALTER TABLE system.%I ENABLE ROW LEVEL SECURITY', t);
        EXECUTE format('ALTER TABLE system.%I FORCE ROW LEVEL SECURITY', t);
        EXECUTE format(
            'CREATE POLICY workspace_isolation ON system.%I USING '
            '(workspace_id = nullif(current_setting(''campbooks.workspace_id'', true), '''')::uuid)',
            t);
        EXECUTE format(
            'CREATE POLICY admin_reads ON system.%I FOR SELECT '
            'USING (current_setting(''campbooks.admin_reads'', true) = ''on'')',
            t);
    END LOOP;
END
$$;

GRANT SELECT, INSERT, UPDATE ON system.ai_usage_days, system.ai_usage TO campbooks_app;
