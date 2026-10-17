SET LOCAL lock_timeout = '5s';

-- The rate-limit prune checks again, on the row it deletes, that its window
-- has ended (campbooks-cloud#213, item 5). The batch is chosen first; a hit
-- that starts a new window on one of its buckets at that moment updates the
-- row, and the outer condition, evaluated on the updated row, now keeps it
-- instead of deleting a live counter (which would hand that caller a fresh
-- budget). Same name, arguments, owner and grants: only the body changes.
CREATE OR REPLACE FUNCTION kernel.prune_rate_limits(batch integer)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, pg_temp
AS $$
DECLARE
    caller_scope text := current_setting('campbooks.kernel_scope', true);
    pruned bigint;
BEGIN
    PERFORM set_config('campbooks.kernel_scope', 'on', true);
    DELETE FROM kernel.rate_limits
     WHERE kernel.rate_limits.bucket IN (
         SELECT e.bucket FROM kernel.rate_limits e
          WHERE e.resets_at <= now()
          LIMIT greatest(coalesce(prune_rate_limits.batch, 0), 0))
       AND kernel.rate_limits.resets_at <= now();
    GET DIAGNOSTICS pruned = ROW_COUNT;
    PERFORM set_config('campbooks.kernel_scope', coalesce(caller_scope, ''), true);
    RETURN pruned;
END
$$;
