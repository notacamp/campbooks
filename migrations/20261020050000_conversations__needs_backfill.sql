-- Wait at most 10 s for a lock (the seed briefly takes `inboxes` exclusively
-- to lift and restore its forced row-level security): a busy instance fails
-- this migration fast, and the next start retries it, rather than stalling.
SET LOCAL lock_timeout = '10s';

-- Needs for mail stored before attention existed (campbooks-cloud#123).
--
-- One row per inbox that exists when this migration runs. The scheduled
-- `conversations.dispatch_needs_backfills` starts each pending row's
-- `conversations.backfill_needs`, which pages through the inbox's recent
-- conversations (and any with an open need) and works out their need from
-- the decisions already stored: no AI call, no rules re-run. Inboxes created
-- later need no row: their needs are raised as their mail is stored.
CREATE TABLE conversations.needs_backfills (
    inbox_id uuid PRIMARY KEY REFERENCES conversations.inboxes (id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL,
    -- `failed`: its page job ran out of attempts; the cursor says where.
    state text NOT NULL DEFAULT 'pending'
        CHECK (state IN ('pending', 'running', 'done', 'failed')),
    -- The last conversation id of the page done so far (pages go by id).
    after_id uuid,
    -- Conversations refreshed so far.
    refreshed bigint NOT NULL DEFAULT 0,
    created_at timestamptz NOT NULL DEFAULT now(),
    finished_at timestamptz
);
CREATE INDEX needs_backfills_pending_idx ON conversations.needs_backfills (created_at)
    WHERE state = 'pending';

-- seed begin
-- Every inbox stored so far, pending. This runs as the migration login,
-- which owns the tables: forced row-level security would hide every inbox
-- from an owner that is not a superuser (no workspace is set here), so it is
-- lifted for this one statement, inside the migration's transaction.
--
-- Starting over is safe (a need that did not change is not announced
-- again): a later migration that changes how needs are worked out re-derives
-- them with this same statement, lifting forced row-level security on
-- `needs_backfills` too.
ALTER TABLE conversations.inboxes NO FORCE ROW LEVEL SECURITY;
INSERT INTO conversations.needs_backfills (inbox_id, workspace_id)
SELECT id, workspace_id FROM conversations.inboxes
ON CONFLICT (inbox_id) DO UPDATE
   SET state = 'pending', after_id = NULL, refreshed = 0, finished_at = NULL;
ALTER TABLE conversations.inboxes FORCE ROW LEVEL SECURITY;
-- seed end

ALTER TABLE conversations.needs_backfills ENABLE ROW LEVEL SECURITY;
ALTER TABLE conversations.needs_backfills FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON conversations.needs_backfills USING
    (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);
-- The dispatcher's maintenance transaction sets no workspace: it may see and
-- start pending rows (ids and states only, no content), nothing else.
CREATE POLICY maintenance_reads_backfills ON conversations.needs_backfills FOR SELECT
    USING (nullif(current_setting('campbooks.workspace_id', true), '') IS NULL);
CREATE POLICY maintenance_starts_backfills ON conversations.needs_backfills FOR UPDATE
    USING (nullif(current_setting('campbooks.workspace_id', true), '') IS NULL)
    WITH CHECK (nullif(current_setting('campbooks.workspace_id', true), '') IS NULL
                AND state = 'running');

-- A row never moves to another inbox or workspace, whoever updates it.
CREATE FUNCTION conversations.needs_backfills_keep_owner() RETURNS trigger
    LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.inbox_id IS DISTINCT FROM OLD.inbox_id
       OR NEW.workspace_id IS DISTINCT FROM OLD.workspace_id THEN
        RAISE EXCEPTION 'needs_backfills rows keep their inbox and workspace'
            USING ERRCODE = 'check_violation';
    END IF;
    RETURN NEW;
END
$$;
CREATE TRIGGER needs_backfills_keep_owner BEFORE UPDATE ON conversations.needs_backfills
    FOR EACH ROW EXECUTE FUNCTION conversations.needs_backfills_keep_owner();

GRANT SELECT, INSERT, UPDATE, DELETE ON conversations.needs_backfills TO campbooks_app;
