SET LOCAL lock_timeout = '5s';

-- A workspace transaction reads no instance-wide row (campbooks-cloud#211).
--
-- `instance_rows_read` (20261021010000) let any transaction working in a
-- workspace read every row of the event log and the queue that belongs to
-- no workspace: every account's events (`identity.signed_out_everywhere`,
-- `identity.account_suspended`, `identity.signed_in`, ... name the account)
-- and their deliveries, the schedules' runs.
--
-- What a workspace transaction needed from it: Postgres checks the read
-- policies against the new row of an `INSERT ... RETURNING` (and of any
-- `INSERT ... ON CONFLICT`), so `EventBus::publish` of an instance-wide event
-- declared `PUBLISHED_IN_WORKSPACES` (identity's `consent_revoked` and
-- `way_in_removed`), which read back the event's id, needed the policy. The
-- kernel now chooses that id itself and reads nothing back; its deliveries
-- were never read back. No workspace transaction enqueues an instance-wide
-- job (the kernel refuses it, #207), so no `ON CONFLICT` there meets an
-- instance-wide row either. Nothing else in a workspace transaction reads
-- one: the worker, pruning, replay and the admin's queue run in the kernel
-- scope, whose `workspace_isolation` admits every row, unchanged.
--
-- Dropping the policy leaves a workspace transaction its own workspace's
-- rows (`workspace_isolation`) and the inserts `instance_rows_insert`
-- admits. Both drops run in this migration's one transaction. A binary
-- from before this change publishes those two events from a workspace with
-- `RETURNING` and is refused until it is replaced; see the change's
-- design.md (openspec/changes/archive/*-kernel-hardening-follow-ups).
DROP POLICY instance_rows_read ON kernel.events;
DROP POLICY instance_rows_read ON kernel.jobs;
