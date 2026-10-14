SET LOCAL lock_timeout = '5s';

-- Row-level security on the kernel's workspace-keyed tables (ADR 0011,
-- `openspec/changes/access-guards`): the change feed, the blob catalogue,
-- the event log and the job queue.
--
-- A transaction that set a workspace (`begin_for`, a job's `ctx.tx()`) sees
-- and writes only that workspace's rows, plus instance-wide rows (NULL
-- workspace) of the event log and the queue. So a request or a job can
-- never publish an event, enqueue a job, record a change or attach a blob
-- for another workspace, nor read one.
--
-- A transaction that set no workspace is the kernel's own machinery working
-- across workspaces: workers claiming jobs fairly, fan-out at publish time,
-- the change sequencer and the feed's reader, the blob sweeper and the
-- workspace purge, pruning, replay. Those keep seeing every row: the
-- kernel's API is their only door, and `cargo xtask arch` keeps modules'
-- queries on workspace tables inside a workspace transaction. Module tables
-- are stricter (no workspace set: no rows) and stay so.
--
-- `kernel.idempotency`, `kernel.rate_limits` and `kernel.schedules` are not
-- keyed by workspace (keys belong to callers, counters to callers, ticks to
-- the instance) and carry no policy.
DO $$
DECLARE
    t text;
BEGIN
    FOREACH t IN ARRAY ARRAY['changes', 'change_heads', 'blobs', 'blob_refs']
    LOOP
        EXECUTE format('ALTER TABLE kernel.%I ENABLE ROW LEVEL SECURITY', t);
        EXECUTE format('ALTER TABLE kernel.%I FORCE ROW LEVEL SECURITY', t);
        EXECUTE format(
            'CREATE POLICY workspace_isolation ON kernel.%I USING '
            '(nullif(current_setting(''campbooks.workspace_id'', true), '''') IS NULL '
            ' OR workspace_id = nullif(current_setting(''campbooks.workspace_id'', true), ''''))',
            t);
    END LOOP;
    FOREACH t IN ARRAY ARRAY['events', 'jobs']
    LOOP
        EXECUTE format('ALTER TABLE kernel.%I ENABLE ROW LEVEL SECURITY', t);
        EXECUTE format('ALTER TABLE kernel.%I FORCE ROW LEVEL SECURITY', t);
        EXECUTE format(
            'CREATE POLICY workspace_isolation ON kernel.%I USING '
            '(nullif(current_setting(''campbooks.workspace_id'', true), '''') IS NULL '
            ' OR workspace_id IS NULL '
            ' OR workspace_id = nullif(current_setting(''campbooks.workspace_id'', true), ''''))',
            t);
    END LOOP;
END
$$;

-- Who may see a change besides the rules of its record type and container:
-- `user` (that user only), `user_and_managers` (that user, the workspace's
-- owners and admins, and client tokens) or `clients` (client tokens only).
-- NULL: everyone the other rules admit.
ALTER TABLE kernel.changes
    ADD COLUMN audience text CHECK (audience IN ('user', 'user_and_managers', 'clients')),
    ADD COLUMN audience_user text,
    ADD CONSTRAINT changes_audience_user_check CHECK (
        CASE WHEN audience IN ('user', 'user_and_managers') THEN audience_user IS NOT NULL
             ELSE audience_user IS NULL END);

-- The containers (`[{type, id}]`) the first request's handler checked
-- access to: a replay is served only to a caller who may still see them.
ALTER TABLE kernel.idempotency
    ADD COLUMN containers jsonb NOT NULL DEFAULT '[]';
