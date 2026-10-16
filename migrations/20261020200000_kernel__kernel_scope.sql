SET LOCAL lock_timeout = '5s';

-- An unset workspace sees nothing (campbooks-cloud#129).
--
-- Until now a transaction that set no workspace saw and wrote every
-- workspace's rows of the kernel's tables: the change feed, the blob
-- catalogue, the event log and the job queue. That was how the kernel's own
-- machinery worked across workspaces, but it also meant any code that forgot
-- `begin_for` (a plain `pool.begin()`, `acquire()`) read and wrote across
-- tenants with nothing to stop it.
--
-- Cross-workspace access now needs the transaction-local setting
-- `campbooks.kernel_scope = 'on'`, which only the kernel sets
-- (`kernel::db::begin_kernel`; `cargo xtask arch` rejects the name anywhere
-- else in Rust and keeps the calls to the kernel and the reviewed
-- `[[access.kernel_scope]]` entries of `architecture.toml`). A transaction
-- with neither a workspace nor the kernel scope sees no row of these tables.
--
-- A workspace transaction keeps seeing its own rows, plus the instance-wide
-- rows (NULL workspace) of the event log and the queue, as before. The
-- kernel writes instance-wide rows for a transaction that set no workspace
-- (an account's event, a scheduled tick) by lifting the scope for its own
-- statement only.
--
-- DROP and CREATE run in this migration's one transaction: no moment exists
-- in which a table has no policy (forced row-level security with no policy
-- would admit nothing, which is safe too).
DO $$
DECLARE
    t text;
BEGIN
    FOREACH t IN ARRAY ARRAY['changes', 'change_heads', 'blobs', 'blob_refs']
    LOOP
        EXECUTE format('DROP POLICY workspace_isolation ON kernel.%I', t);
        EXECUTE format(
            'CREATE POLICY workspace_isolation ON kernel.%I USING '
            '(current_setting(''campbooks.kernel_scope'', true) = ''on'' '
            ' OR workspace_id = nullif(current_setting(''campbooks.workspace_id'', true), ''''))',
            t);
    END LOOP;
    FOREACH t IN ARRAY ARRAY['events', 'jobs']
    LOOP
        EXECUTE format('DROP POLICY workspace_isolation ON kernel.%I', t);
        EXECUTE format(
            'CREATE POLICY workspace_isolation ON kernel.%I USING '
            '(current_setting(''campbooks.kernel_scope'', true) = ''on'' '
            ' OR (workspace_id IS NULL '
            '     AND nullif(current_setting(''campbooks.workspace_id'', true), '''') IS NOT NULL) '
            ' OR workspace_id = nullif(current_setting(''campbooks.workspace_id'', true), ''''))',
            t);
    END LOOP;
END
$$;
