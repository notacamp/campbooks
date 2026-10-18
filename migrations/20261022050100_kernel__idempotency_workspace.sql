SET LOCAL lock_timeout = '5s';

-- Idempotency keys name the workspace of their route, and go with its purge
-- (campbooks-cloud#65).
--
-- A stored answer can hold what the operation returned (a draft, a
-- contact's name) for the key's day. When a workspace is purged, its keys
-- now go too, even unexpired ones (after a database restore, the purge runs
-- on keys restored with it).
--
-- 1. `workspace_id`: the `{workspace_id}` segment of the matched route the
--    key was first used on, set by the idempotency layer from the route
--    alone, never from the request's body or query. NULL for a route
--    outside a workspace (those keys keep expiring after a day).
ALTER TABLE kernel.idempotency ADD COLUMN workspace_id text;
CREATE INDEX idempotency_workspace_idx ON kernel.idempotency (workspace_id)
    WHERE workspace_id IS NOT NULL;

-- 2. The purge. Keys stay their caller's: no workspace transaction may read
--    or write them (campbooks-cloud#210). A workspace's purge deletes its
--    keys through `kernel.purge_workspace_idempotency(workspace, batch)`, a
--    definer function owned by `campbooks_definer` (as the rate-limit and
--    rule-resign functions are). It refuses unless the calling transaction
--    works in that very workspace, deletes up to `batch` of its keys, and
--    answers how many. It reads nothing back to the caller.
--
--    Row-level security stays forced and applies to the owner role too:
--    the two policies below admit `campbooks_definer` (and no other role)
--    to the keys of the transaction's workspace, to find and delete them.
CREATE POLICY definer_finds_workspace_keys ON kernel.idempotency FOR SELECT
    TO campbooks_definer
    USING (workspace_id = nullif(current_setting('campbooks.workspace_id', true), ''));
CREATE POLICY definer_purges_workspace_keys ON kernel.idempotency FOR DELETE
    TO campbooks_definer
    USING (workspace_id = nullif(current_setting('campbooks.workspace_id', true), ''));

CREATE FUNCTION kernel.purge_workspace_idempotency(workspace text, batch integer)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, pg_temp
AS $$
DECLARE
    purged bigint;
BEGIN
    IF purge_workspace_idempotency.workspace IS NULL
       OR purge_workspace_idempotency.workspace
          IS DISTINCT FROM nullif(current_setting('campbooks.workspace_id', true), '') THEN
        RAISE EXCEPTION 'a workspace''s idempotency keys are purged in a transaction working '
                        'in that workspace'
            USING ERRCODE = '42501';
    END IF;
    DELETE FROM kernel.idempotency i
     WHERE (i.principal, i.key) IN (
         SELECT e.principal, e.key FROM kernel.idempotency e
          WHERE e.workspace_id = purge_workspace_idempotency.workspace
          LIMIT greatest(coalesce(purge_workspace_idempotency.batch, 0), 0));
    GET DIAGNOSTICS purged = ROW_COUNT;
    RETURN purged;
END
$$;

-- What the function's statements need, and nothing else.
GRANT SELECT, DELETE ON kernel.idempotency TO campbooks_definer;

-- The hand-over (as in `…080000_kernel__definer_owner_role`): the right to
-- create in the schema is lent for it and taken back in this transaction.
GRANT CREATE ON SCHEMA kernel TO campbooks_definer;
ALTER FUNCTION kernel.purge_workspace_idempotency(text, integer) OWNER TO campbooks_definer;
REVOKE CREATE ON SCHEMA kernel FROM campbooks_definer;

REVOKE ALL ON FUNCTION kernel.purge_workspace_idempotency(text, integer) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION kernel.purge_workspace_idempotency(text, integer) TO campbooks_app;
