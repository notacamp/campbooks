SET LOCAL lock_timeout = '5s';

-- Job unique and ordering keys are per workspace (campbooks-cloud#209):
-- drop the old cross-workspace indexes now that 20261021130000 and
-- 20261021130100 built their per-workspace replacements.
--
-- Dropping an index takes an ACCESS EXCLUSIVE lock on `kernel.jobs` for a
-- catalog change only (no scan, milliseconds). Behind a long-running
-- transaction the lock timeout fails the migration (both index sets still
-- exist, so nothing breaks) and the server migrates again at its next
-- start, rather than queue every enqueue behind the wait.
--
-- From here on a binary from before this change cannot enqueue a job with
-- a unique key: its `ON CONFLICT (kind, unique_key)` names no index any
-- more and Postgres refuses the statement (42P10). Run the API and the
-- worker of the same side; to go back, run the rollback SQL in the change's
-- design.md first (openspec/changes/archive/*-job-keys-per-workspace).
DROP INDEX kernel.jobs_unique_key_idx;
DROP INDEX kernel.jobs_ordering_idx;
