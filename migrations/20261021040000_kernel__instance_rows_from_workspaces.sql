SET LOCAL lock_timeout = '5s';

-- Workspaces may no longer forge instance-wide work (campbooks-cloud#207).
--
-- After 20261021010000 a workspace transaction could still INSERT any
-- instance-wide row (no workspace) into the event log and the queue, and any
-- row of its own with any unique key. A raw INSERT of
-- `kind = 'kernel.prune_jobs', unique_key = 'schedule:kernel.prune_jobs',
-- run_at = 'infinity'` made every later tick of that schedule a duplicate
-- (`ON CONFLICT DO NOTHING`): pruning stopped, and the same row for
-- `connectors.dispatch_due` stopped every stream's polling. The unique index
-- is `(kind, unique_key)` across workspaces, so the workspace's own rows
-- could do it too.
--
-- The only instance-wide rows a workspace transaction legitimately adds are
-- identity's account events announced by a workspace handler
-- (`identity.consent_revoked`, from removing a workspace client or from a
-- departed member's consents; `identity.way_in_removed`, from a removed
-- mailbox's last way in) and their deliveries. Those event names, and the
-- job kinds of their consumers (none today), are listed in a kernel table
-- the policies read. The runtime role may only read it: what a workspace
-- may add changes by migration, reviewed, never at runtime. The server's
-- test keeps the table equal to what the registry derives (events declared
-- `DomainEvent::PUBLISHED_IN_WORKSPACES` and their consumers).
CREATE TABLE kernel.instance_rows_from_workspaces (
    -- 'event': a `kernel.events.name`; 'job': a `kernel.jobs.kind`.
    row_kind text NOT NULL CHECK (row_kind IN ('event', 'job')),
    name text NOT NULL,
    PRIMARY KEY (row_kind, name)
);

GRANT SELECT ON kernel.instance_rows_from_workspaces TO campbooks_app;

INSERT INTO kernel.instance_rows_from_workspaces (row_kind, name) VALUES
    ('event', 'identity.consent_revoked'),
    ('event', 'identity.way_in_removed');

-- The policies, replaced in this one transaction (no moment with fewer
-- policies than either side):
--
-- - `workspace_isolation` (`FOR ALL`) keeps its `USING` (the kernel scope, or
--   the transaction's own workspace) and gains a `WITH CHECK`: outside the
--   kernel scope no row, a workspace's own included, may carry a schedule's
--   unique key (`schedule:<name>`), which only the schedule tick sets.
-- - `instance_rows_insert` (`FOR INSERT`): an instance-wide row from a
--   workspace transaction only as a fresh event of a listed name, or a fresh
--   delivery of an event to a listed consumer: pending, unclaimed, due now,
--   keyed by its event like `EventBus::publish` writes it, no ordering key,
--   no schedule key.
-- - `instance_rows_read` (`FOR SELECT`) is unchanged.
-- The delivery is not checked against its event's row: `EventBus::publish`
-- writes both in one statement, whose policy subqueries cannot see the
-- event it is inserting; `load_event` refuses a delivery whose event was
-- not logged where its type says (an account's event in a workspace).
DROP POLICY workspace_isolation ON kernel.jobs;
CREATE POLICY workspace_isolation ON kernel.jobs
    USING (current_setting('campbooks.kernel_scope', true) = 'on'
           OR workspace_id = nullif(current_setting('campbooks.workspace_id', true), ''))
    WITH CHECK (current_setting('campbooks.kernel_scope', true) = 'on'
                OR (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')
                    AND (unique_key IS NULL OR unique_key NOT LIKE 'schedule:%')));

DROP POLICY instance_rows_insert ON kernel.jobs;
CREATE POLICY instance_rows_insert ON kernel.jobs FOR INSERT
    WITH CHECK (workspace_id IS NULL
                AND nullif(current_setting('campbooks.workspace_id', true), '') IS NOT NULL
                AND state = 'pending'
                AND attempts = 0
                AND locked_by IS NULL
                AND locked_until IS NULL
                AND started_at IS NULL
                AND finished_at IS NULL
                AND last_error IS NULL
                AND run_at <= now()
                AND run_at >= now() - interval '5 minutes'
                AND payload = '{}'::jsonb
                AND event_id IS NOT NULL
                AND unique_key = event_id::text
                AND unique_key NOT LIKE 'schedule:%'
                AND ordering_key IS NULL
                AND EXISTS (SELECT 1 FROM kernel.instance_rows_from_workspaces a
                             WHERE a.row_kind = 'job' AND a.name = jobs.kind));

DROP POLICY instance_rows_insert ON kernel.events;
CREATE POLICY instance_rows_insert ON kernel.events FOR INSERT
    WITH CHECK (workspace_id IS NULL
                AND nullif(current_setting('campbooks.workspace_id', true), '') IS NOT NULL
                AND occurred_at <= now()
                AND occurred_at >= now() - interval '5 minutes'
                AND EXISTS (SELECT 1 FROM kernel.instance_rows_from_workspaces a
                             WHERE a.row_kind = 'event' AND a.name = events.name));

-- The schedules' last ticks: only the schedule tick (the kernel scope) reads
-- and writes them. Before this, the runtime role could set a schedule's
-- `last_tick` far ahead from any transaction, and every later tick of that
-- schedule found itself already done (`WHERE last_tick < excluded.last_tick`):
-- pruning, polling, sequencing and the backfills stopped, before the heal
-- above could run. The tick also resets a `last_tick` more than one period
-- ahead.
ALTER TABLE kernel.schedules ENABLE ROW LEVEL SECURITY;
ALTER TABLE kernel.schedules FORCE ROW LEVEL SECURITY;
CREATE POLICY kernel_scope_only ON kernel.schedules
    USING (current_setting('campbooks.kernel_scope', true) = 'on')
    WITH CHECK (current_setting('campbooks.kernel_scope', true) = 'on');
