SET LOCAL lock_timeout = '5s';

-- The job queue's per-workspace key indexes are valid (campbooks-cloud#213,
-- item 14).
--
-- 20261021130000 and 20261021130100 build them CONCURRENTLY, which leaves
-- an INVALID index behind when the build fails or its process dies.
-- Before migrating, the server drops invalid indexes; `cargo sqlx migrate
-- run` (development, CI, a self-hoster migrating by hand) does not. There,
-- an invalid index kept its name, `IF NOT EXISTS` passed over it, the
-- migration was recorded, and 20261021130200 then dropped the old indexes:
-- every enqueue with a unique key failed (42P10, no index to infer the
-- conflict from), and ordered claims scanned the queue.
--
-- Each index missing or not valid is built again here, in this transaction:
-- not concurrently, so enqueues wait for the build, which only happens on an
-- installation in that broken state (anywhere else this does nothing). An
-- invalid unique index enforced nothing for inference, so no duplicate
-- unfinished keys can have been added since: the build cannot fail on them.
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1
                     FROM pg_index i
                     JOIN pg_class c ON c.oid = i.indexrelid
                     JOIN pg_namespace n ON n.oid = c.relnamespace
                    WHERE n.nspname = 'kernel'
                      AND c.relname = 'jobs_unique_key_per_workspace_idx'
                      AND i.indisvalid) THEN
        RAISE WARNING 'rebuilding kernel.jobs_unique_key_per_workspace_idx (missing or invalid)';
        DROP INDEX IF EXISTS kernel.jobs_unique_key_per_workspace_idx;
        CREATE UNIQUE INDEX jobs_unique_key_per_workspace_idx
            ON kernel.jobs (kind, unique_key, workspace_id) NULLS NOT DISTINCT
            WHERE unique_key IS NOT NULL AND state IN ('pending', 'running');
    END IF;
    IF NOT EXISTS (SELECT 1
                     FROM pg_index i
                     JOIN pg_class c ON c.oid = i.indexrelid
                     JOIN pg_namespace n ON n.oid = c.relnamespace
                    WHERE n.nspname = 'kernel'
                      AND c.relname = 'jobs_ordering_per_workspace_idx'
                      AND i.indisvalid) THEN
        RAISE WARNING 'rebuilding kernel.jobs_ordering_per_workspace_idx (missing or invalid)';
        DROP INDEX IF EXISTS kernel.jobs_ordering_per_workspace_idx;
        CREATE INDEX jobs_ordering_per_workspace_idx
            ON kernel.jobs (ordering_key, workspace_key, seq)
            WHERE ordering_key IS NOT NULL AND state IN ('pending', 'running');
    END IF;
END
$$;
