SET LOCAL lock_timeout = '5s';

-- The relay of the tracked events outbox (ADR 0020, campbooks-cloud#321).
--
-- The runtime role may only append to `kernel.tracked_events` (#320): it
-- cannot read, change or delete a row. The relay, the hourly prune, the
-- metrics sample and a workspace's purge reach the rows through the
-- functions below, owned by `campbooks_definer` (as the kernel's other
-- definer functions are), with their search path pinned. Each says what it
-- refuses:
--
-- - `kernel.claim_tracked_events(batch)`: up to `batch` unpublished rows,
--   oldest id first, locked `FOR UPDATE SKIP LOCKED` in the caller's
--   transaction, so two relays never hold the same row. Kernel scope only.
-- - `kernel.mark_tracked_events_published(ids)`: sets `published_at` on
--   those of `ids` still unpublished, in the same transaction, once the
--   broker confirmed them. Kernel scope only.
-- - `kernel.tracked_events_backlog()`: how many rows wait, and the age of
--   the oldest, for the metrics. Kernel scope only.
-- - `kernel.prune_tracked_events(retention_hours, batch)`: deletes up to
--   `batch` rows published more than `retention_hours` ago. Never an
--   unpublished row. Kernel scope only.
-- - `kernel.purge_workspace_tracked_events(workspace, batch)`: deletes up
--   to `batch` of a purged workspace's rows, published or not, in a
--   transaction working in that very workspace (as
--   `kernel.purge_workspace_idempotency` does).
--
-- The kernel scope (`campbooks.kernel_scope`) is set only by the kernel
-- (`kernel::db::begin_kernel`), so a module's workspace transaction cannot
-- read another workspace's events through these. Row-level security stays
-- forced: `workspace_isolation` admits every row in the kernel scope, and
-- the transaction's workspace's rows otherwise, to the owner role too.

CREATE FUNCTION kernel.claim_tracked_events(batch integer)
    RETURNS TABLE (
        id uuid,
        name text,
        version integer,
        occurred_at timestamptz,
        envelope jsonb,
        body jsonb
    )
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, pg_temp
AS $$
BEGIN
    IF current_setting('campbooks.kernel_scope', true) IS DISTINCT FROM 'on' THEN
        RAISE EXCEPTION 'tracked events are claimed in the kernel scope'
            USING ERRCODE = '42501';
    END IF;
    RETURN QUERY
        SELECT t.id, t.name, t.version, t.occurred_at, t.envelope, t.body
          FROM kernel.tracked_events t
         WHERE t.published_at IS NULL
         ORDER BY t.id
         LIMIT greatest(coalesce(claim_tracked_events.batch, 0), 0)
           FOR UPDATE SKIP LOCKED;
END
$$;

CREATE FUNCTION kernel.mark_tracked_events_published(ids uuid[])
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, pg_temp
AS $$
DECLARE
    marked bigint;
BEGIN
    IF current_setting('campbooks.kernel_scope', true) IS DISTINCT FROM 'on' THEN
        RAISE EXCEPTION 'tracked events are marked published in the kernel scope'
            USING ERRCODE = '42501';
    END IF;
    UPDATE kernel.tracked_events t
       SET published_at = now()
     WHERE t.id = ANY (mark_tracked_events_published.ids)
       AND t.published_at IS NULL;
    GET DIAGNOSTICS marked = ROW_COUNT;
    RETURN marked;
END
$$;

CREATE FUNCTION kernel.tracked_events_backlog()
    RETURNS TABLE (waiting bigint, oldest_seconds double precision)
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, pg_temp
AS $$
BEGIN
    IF current_setting('campbooks.kernel_scope', true) IS DISTINCT FROM 'on' THEN
        RAISE EXCEPTION 'the tracked events backlog is read in the kernel scope'
            USING ERRCODE = '42501';
    END IF;
    RETURN QUERY
        SELECT count(*),
               coalesce(extract(epoch FROM now() - min(t.occurred_at))::double precision, 0)
          FROM kernel.tracked_events t
         WHERE t.published_at IS NULL;
END
$$;

CREATE FUNCTION kernel.prune_tracked_events(retention_hours integer, batch integer)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, pg_temp
AS $$
DECLARE
    pruned bigint;
BEGIN
    IF current_setting('campbooks.kernel_scope', true) IS DISTINCT FROM 'on' THEN
        RAISE EXCEPTION 'tracked events are pruned in the kernel scope'
            USING ERRCODE = '42501';
    END IF;
    -- The outer condition is checked again on the row it deletes: a row
    -- that is not published (yet) is never deleted.
    DELETE FROM kernel.tracked_events t
     WHERE t.id IN (
         SELECT e.id FROM kernel.tracked_events e
          WHERE e.published_at IS NOT NULL
            AND e.published_at
                < now() - make_interval(hours => greatest(coalesce(prune_tracked_events.retention_hours, 0), 1))
          LIMIT greatest(coalesce(prune_tracked_events.batch, 0), 0))
       AND t.published_at IS NOT NULL;
    GET DIAGNOSTICS pruned = ROW_COUNT;
    RETURN pruned;
END
$$;

CREATE FUNCTION kernel.purge_workspace_tracked_events(workspace text, batch integer)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, pg_temp
AS $$
DECLARE
    purged bigint;
BEGIN
    IF purge_workspace_tracked_events.workspace IS NULL
       OR purge_workspace_tracked_events.workspace
          IS DISTINCT FROM nullif(current_setting('campbooks.workspace_id', true), '') THEN
        RAISE EXCEPTION 'a workspace''s tracked events are purged in a transaction working in '
                        'that workspace'
            USING ERRCODE = '42501';
    END IF;
    DELETE FROM kernel.tracked_events t
     WHERE t.id IN (
         SELECT e.id FROM kernel.tracked_events e
          WHERE e.workspace_id::text = purge_workspace_tracked_events.workspace
          LIMIT greatest(coalesce(purge_workspace_tracked_events.batch, 0), 0));
    GET DIAGNOSTICS purged = ROW_COUNT;
    RETURN purged;
END
$$;

-- What the functions' statements need, and nothing else.
GRANT SELECT, UPDATE, DELETE ON kernel.tracked_events TO campbooks_definer;

-- The hand-over (as in `…080000_kernel__definer_owner_role`): the right to
-- create in the schema is lent for it and taken back in this transaction.
GRANT CREATE ON SCHEMA kernel TO campbooks_definer;
ALTER FUNCTION kernel.claim_tracked_events(integer) OWNER TO campbooks_definer;
ALTER FUNCTION kernel.mark_tracked_events_published(uuid[]) OWNER TO campbooks_definer;
ALTER FUNCTION kernel.tracked_events_backlog() OWNER TO campbooks_definer;
ALTER FUNCTION kernel.prune_tracked_events(integer, integer) OWNER TO campbooks_definer;
ALTER FUNCTION kernel.purge_workspace_tracked_events(text, integer) OWNER TO campbooks_definer;
REVOKE CREATE ON SCHEMA kernel FROM campbooks_definer;

REVOKE ALL ON FUNCTION kernel.claim_tracked_events(integer) FROM PUBLIC;
REVOKE ALL ON FUNCTION kernel.mark_tracked_events_published(uuid[]) FROM PUBLIC;
REVOKE ALL ON FUNCTION kernel.tracked_events_backlog() FROM PUBLIC;
REVOKE ALL ON FUNCTION kernel.prune_tracked_events(integer, integer) FROM PUBLIC;
REVOKE ALL ON FUNCTION kernel.purge_workspace_tracked_events(text, integer) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION kernel.claim_tracked_events(integer) TO campbooks_app;
GRANT EXECUTE ON FUNCTION kernel.mark_tracked_events_published(uuid[]) TO campbooks_app;
GRANT EXECUTE ON FUNCTION kernel.tracked_events_backlog() TO campbooks_app;
GRANT EXECUTE ON FUNCTION kernel.prune_tracked_events(integer, integer) TO campbooks_app;
GRANT EXECUTE ON FUNCTION kernel.purge_workspace_tracked_events(text, integer) TO campbooks_app;

-- Wake the relay when events are tracked: one empty notification on
-- `kernel_tracked_events` per inserting statement (Postgres folds identical
-- notifications of a transaction into one, delivered at commit; a rolled
-- back insert sends none). The relay also polls, so a missed notification
-- only delays a batch. `pg_notify` with a fixed channel and an empty payload
-- raises nothing at insert time.
CREATE FUNCTION kernel.notify_tracked_events()
    RETURNS trigger
    LANGUAGE plpgsql
    SET search_path = pg_catalog, pg_temp
AS $$
BEGIN
    PERFORM pg_notify('kernel_tracked_events', '');
    RETURN NULL;
END
$$;
-- (A trigger function cannot be called on its own, so it keeps the default
-- grant.)

CREATE TRIGGER tracked_events_notify
    AFTER INSERT ON kernel.tracked_events
    FOR EACH STATEMENT
    EXECUTE FUNCTION kernel.notify_tracked_events();
