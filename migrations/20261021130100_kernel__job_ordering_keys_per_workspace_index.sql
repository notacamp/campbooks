-- no-transaction
-- Job ordering keys are per workspace (campbooks-cloud#209): a job waits
-- only for earlier unfinished jobs of its own workspace (or, instance-wide,
-- of no workspace) sharing its ordering key. The claim's check compares
-- `workspace_key` (`coalesce(workspace_id, '')`, the fairness bucket) so
-- this index serves it. A workspace id is never empty (outside the kernel
-- scope row-level security admits no workspace `''`), and a collision could
-- only make a job wait longer, never run out of order.
--
-- Built CONCURRENTLY in a migration of its own, like 20261021130000.
-- 20261021130200 drops the old `(ordering_key, seq)` index.
CREATE INDEX CONCURRENTLY IF NOT EXISTS jobs_ordering_per_workspace_idx
    ON kernel.jobs (ordering_key, workspace_key, seq)
    WHERE ordering_key IS NOT NULL AND state IN ('pending', 'running');
