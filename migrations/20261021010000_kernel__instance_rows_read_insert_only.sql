SET LOCAL lock_timeout = '5s';

-- Instance-wide rows change only in the kernel scope (campbooks-cloud#196).
--
-- The event log's and the queue's `workspace_isolation` policy (`FOR ALL`,
-- 20261020200000) let a workspace transaction reach the instance-wide rows
-- (no workspace: an account's events and jobs, a schedule's ticks) for every
-- command, so it could also UPDATE and DELETE them: complete, requeue or
-- delete another job, rewrite an account's event.
--
-- Split by command:
-- - `workspace_isolation` (`FOR ALL`): the kernel scope, or the
--   transaction's own workspace. The only policy for UPDATE and DELETE.
-- - `instance_rows_read` (`FOR SELECT`): a workspace transaction still sees
--   the instance-wide rows. A unique-key enqueue (`ON CONFLICT`), an
--   instance-wide follow-up of a workspace job and the `RETURNING` of an
--   insert need it.
-- - `instance_rows_insert` (`FOR INSERT`): a workspace transaction may still
--   add an instance-wide row (a workspace handler's instance-wide event or
--   job).
--
-- Every UPDATE and DELETE of an instance-wide row already runs in the kernel
-- scope (the worker, pruning, the admin queue); see the change's design.md.
-- DROP and CREATE run in this migration's one transaction: no moment exists
-- in which a table has fewer policies than either side of the change.
DO $$
DECLARE
    t text;
BEGIN
    FOREACH t IN ARRAY ARRAY['events', 'jobs']
    LOOP
        EXECUTE format('DROP POLICY workspace_isolation ON kernel.%I', t);
        EXECUTE format(
            'CREATE POLICY workspace_isolation ON kernel.%I USING '
            '(current_setting(''campbooks.kernel_scope'', true) = ''on'' '
            ' OR workspace_id = nullif(current_setting(''campbooks.workspace_id'', true), ''''))',
            t);
        EXECUTE format(
            'CREATE POLICY instance_rows_read ON kernel.%I FOR SELECT USING '
            '(workspace_id IS NULL '
            ' AND nullif(current_setting(''campbooks.workspace_id'', true), '''') IS NOT NULL)',
            t);
        EXECUTE format(
            'CREATE POLICY instance_rows_insert ON kernel.%I FOR INSERT WITH CHECK '
            '(workspace_id IS NULL '
            ' AND nullif(current_setting(''campbooks.workspace_id'', true), '''') IS NOT NULL)',
            t);
    END LOOP;
END
$$;
