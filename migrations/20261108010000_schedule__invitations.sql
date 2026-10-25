-- Invitations awaiting the member's answer on Home (campbooks-cloud#360).
--
-- An event on one of the member's provider calendars that syncs, where the
-- member is asked (`response = 'needs_action'`), that the member did not
-- organise, is not cancelled or deleted and has not started, is announced as
-- `schedule.invitation_changed` v1, which the attention module projects into
-- a Decide item. To tell who asked, the event keeps its organiser as the
-- provider gave it (`organizer_name`, `organizer_address`, whether it is the
-- member, and whether a person rather than a machine sent it, from the
-- address alone). `invitation_revision` grows with every announcement, so
-- the projection applies only newer ones; `invitation_open` says whether the
-- last one said it awaits an answer, so a close is announced only over an
-- open one.
--
-- Then every workspace with an unanswered event is queued in
-- `invitation_backfills`: the minute dispatcher
-- (`schedule.dispatch_invitation_backfills`) has
-- `schedule.announce_invitations` announce each one once, from this
-- module's own rows, with no resync and no model. The organiser of an event
-- stored before this migration is learnt with its next change from the
-- provider. Idempotent: a second run queues the workspaces again, which
-- announces the same invitations again with a newer revision.

-- Wait at most 10 s for a lock: a busy instance fails this migration fast,
-- and the next start retries it, rather than stalling.
SET LOCAL lock_timeout = '10s';

-- Catalog-only: nullable columns and constant defaults.
ALTER TABLE schedule.events
    ADD COLUMN organizer_name text,
    ADD COLUMN organizer_address text,
    ADD COLUMN organizer_self boolean NOT NULL DEFAULT false,
    ADD COLUMN organizer_person boolean,
    ADD COLUMN invitation_revision bigint NOT NULL DEFAULT 0,
    ADD COLUMN invitation_open boolean NOT NULL DEFAULT false;

-- Workspaces whose invitations are still to be announced once. Ids only.
CREATE TABLE schedule.invitation_backfills (
    workspace_id uuid PRIMARY KEY,
    created_at timestamptz NOT NULL DEFAULT now()
);

ALTER TABLE schedule.invitation_backfills ENABLE ROW LEVEL SECURITY;
ALTER TABLE schedule.invitation_backfills FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON schedule.invitation_backfills USING
    (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);
-- The dispatcher reads which workspaces still have a row (ids only).
CREATE POLICY maintenance_reads_invitation_backfills ON schedule.invitation_backfills
    FOR SELECT USING (current_setting('campbooks.maintenance_scope', true) = 'on');

-- The workspace's job reads its row and removes it once announced.
GRANT SELECT, DELETE ON schedule.invitation_backfills TO campbooks_app;

-- seed begin
-- Both tables force row-level security even for their owner, and no
-- workspace is set here: it is lifted for this statement only, inside the
-- migration's transaction.
ALTER TABLE schedule.events NO FORCE ROW LEVEL SECURITY;
ALTER TABLE schedule.invitation_backfills NO FORCE ROW LEVEL SECURITY;
INSERT INTO schedule.invitation_backfills (workspace_id)
SELECT DISTINCT workspace_id FROM schedule.events WHERE response = 'needs_action'
ON CONFLICT (workspace_id) DO NOTHING;
ALTER TABLE schedule.invitation_backfills FORCE ROW LEVEL SECURITY;
ALTER TABLE schedule.events FORCE ROW LEVEL SECURITY;
-- seed end
