-- Which workspaces have a scheduled send past its time (campbooks-cloud#392):
-- the overdue-sends dispatcher (`conversations.dispatch_overdue_sends`) runs
-- in no workspace, and asks only this, so it wakes the sweep of each
-- workspace with work to do and no other.
--
-- Ids only. The function runs as `campbooks_definer` (never the migration
-- login, see `20261021080000_kernel__definer_owner_role`), which may read
-- three columns of `conversations.drafts` (`workspace_id`, `state`,
-- `send_at`) and nothing else, through a policy for that role alone: no
-- maintenance policy that would let a transaction read whole drafts (their
-- words and addresses). Only the runtime role may call it.
--
-- And the partial index both the function and the per-workspace sweep read:
-- the few drafts that are scheduled, by when they leave.
SET LOCAL lock_timeout = '5s';

CREATE INDEX IF NOT EXISTS drafts_scheduled_send_at_idx
    ON conversations.drafts (send_at) WHERE state = 'scheduled';

-- Up to `max` workspaces with a draft still scheduled to leave before
-- `cutoff`.
CREATE OR REPLACE FUNCTION conversations.workspaces_with_overdue_sends(cutoff timestamptz,
                                                                       max integer)
    RETURNS SETOF uuid
    LANGUAGE sql
    STABLE
    SECURITY DEFINER
    SET search_path = pg_catalog, pg_temp
AS $$
    SELECT DISTINCT workspace_id FROM conversations.drafts
     WHERE state = 'scheduled' AND send_at < cutoff
     LIMIT greatest(least(max, 10000), 0)
$$;

-- What the function's statement reads, and nothing more.
GRANT USAGE ON SCHEMA conversations TO campbooks_definer;
GRANT SELECT (workspace_id, state, send_at) ON conversations.drafts TO campbooks_definer;
CREATE POLICY definer_overdue_sends ON conversations.drafts FOR SELECT TO campbooks_definer
    USING (true);

-- The hand-over (the right to create in the schema is lent for it).
GRANT CREATE ON SCHEMA conversations TO campbooks_definer;
ALTER FUNCTION conversations.workspaces_with_overdue_sends(timestamptz, integer)
    OWNER TO campbooks_definer;
REVOKE CREATE ON SCHEMA conversations FROM campbooks_definer;

-- The runtime role may call it, and nothing else may.
REVOKE ALL ON FUNCTION conversations.workspaces_with_overdue_sends(timestamptz, integer)
    FROM PUBLIC;
GRANT EXECUTE ON FUNCTION conversations.workspaces_with_overdue_sends(timestamptz, integer)
    TO campbooks_app;
