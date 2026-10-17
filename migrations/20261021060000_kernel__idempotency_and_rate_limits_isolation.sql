SET LOCAL lock_timeout = '5s';

-- Idempotency keys and rate-limit counters are isolated (campbooks-cloud#210).
--
-- Both tables had no row-level security and the runtime role could read and
-- write every row from any transaction. `kernel.idempotency` kept each
-- caller's stored responses, the one-time secret of `register_client`'s 201
-- included: a workspace transaction could read another caller's cached
-- secret, or forge a response a later replay would serve. Anyone could reset
-- or exhaust any rate-limit counter.

-- 1. Cached responses that may hold a one-time secret or ticket are dropped.
--    They are a day's cache of answers, not user data: a retry with the same
--    key runs the operation again. Done before row-level security is on, so
--    a migration login that is not a superuser still sees every row.
DELETE FROM kernel.idempotency
 WHERE operation_id IN ('identity.register_client',
                        'identity.rotate_client_secret',
                        'workspaces.invite_member',
                        'identity.start_link',
                        'connectors.start_connection');

-- 2. A key whose first answer showed a secret is kept as `withheld`: the
--    status and the location only, never the body. A replay answers 409
--    `idempotency_secret_shown_once`.
ALTER TABLE kernel.idempotency DROP CONSTRAINT idempotency_state_check;
ALTER TABLE kernel.idempotency
    ADD CONSTRAINT idempotency_state_check
        CHECK (state IN ('pending', 'completed', 'withheld')),
    ADD CONSTRAINT idempotency_withheld_body_check
        CHECK (state <> 'withheld'
               OR (response_body = ''::bytea AND response_content_type IS NULL));

-- 3. Keys belong to their caller (`user:<subject>`, `client:<id>`), and only
--    the idempotency layer, in a transaction naming that caller
--    (`campbooks.idempotency_caller`, set only by the kernel), reaches them.
--    The kernel scope (the hourly prune) may read and delete expired keys,
--    nothing else. A workspace transaction, or one that set nothing, sees no
--    key and can write none.
ALTER TABLE kernel.idempotency ENABLE ROW LEVEL SECURITY;
ALTER TABLE kernel.idempotency FORCE ROW LEVEL SECURITY;
CREATE POLICY caller_isolation ON kernel.idempotency
    USING (principal = nullif(current_setting('campbooks.idempotency_caller', true), ''))
    WITH CHECK (principal = nullif(current_setting('campbooks.idempotency_caller', true), ''));
CREATE POLICY kernel_scope_reads_expired ON kernel.idempotency FOR SELECT
    USING (current_setting('campbooks.kernel_scope', true) = 'on' AND expires_at <= now());
CREATE POLICY kernel_scope_prunes_expired ON kernel.idempotency FOR DELETE
    USING (current_setting('campbooks.kernel_scope', true) = 'on' AND expires_at <= now());

-- 4. Rate-limit counters: the runtime role loses every privilege on the
--    table and counts through two functions instead, which run as the
--    table's owner (`SECURITY DEFINER`, a pinned `search_path`) and turn the
--    kernel scope on for their own statement only, restoring the caller's
--    setting before they return (an error aborts the caller's statement, or
--    its savepoint, which undoes the setting too):
--    - `kernel.take_rate_limit(bucket, window_seconds, cost)` adds `cost`
--      hits to one bucket (an HMAC under a key derived from the instance
--      secret, which the database never sees) and answers its hits and the
--      seconds left in its window;
--    - `kernel.prune_rate_limits(batch)` deletes up to `batch` ended windows.
--    No caller can list, read, lower or reset a counter, or open a window
--    longer than a day. Row-level security (kernel scope only) covers the
--    table for the owner and for any role given privileges later.
REVOKE ALL ON kernel.rate_limits FROM campbooks_app;
ALTER TABLE kernel.rate_limits ENABLE ROW LEVEL SECURITY;
ALTER TABLE kernel.rate_limits FORCE ROW LEVEL SECURITY;
CREATE POLICY kernel_scope_only ON kernel.rate_limits
    USING (current_setting('campbooks.kernel_scope', true) = 'on')
    WITH CHECK (current_setting('campbooks.kernel_scope', true) = 'on');

CREATE FUNCTION kernel.take_rate_limit(bucket bytea, window_seconds double precision,
                                       cost integer)
    RETURNS TABLE (hits integer, retry_after bigint)
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, pg_temp
AS $$
#variable_conflict use_column
DECLARE
    caller_scope text := current_setting('campbooks.kernel_scope', true);
BEGIN
    IF take_rate_limit.bucket IS NULL OR octet_length(take_rate_limit.bucket) <> 32 THEN
        RAISE EXCEPTION 'a rate-limit bucket is 32 bytes' USING ERRCODE = '22023';
    END IF;
    IF take_rate_limit.window_seconds IS NULL
       OR NOT (take_rate_limit.window_seconds > 0 AND take_rate_limit.window_seconds <= 86400) THEN
        RAISE EXCEPTION 'a rate-limit window is more than 0 and at most 86400 seconds'
            USING ERRCODE = '22023';
    END IF;
    IF take_rate_limit.cost IS NULL OR take_rate_limit.cost < 1 THEN
        RAISE EXCEPTION 'a rate-limit hit costs at least 1' USING ERRCODE = '22023';
    END IF;
    PERFORM set_config('campbooks.kernel_scope', 'on', true);
    RETURN QUERY
    INSERT INTO kernel.rate_limits AS r (bucket, hits, resets_at)
    VALUES (take_rate_limit.bucket, take_rate_limit.cost,
            now() + make_interval(secs => take_rate_limit.window_seconds))
    ON CONFLICT ON CONSTRAINT rate_limits_pkey DO UPDATE SET
        hits = CASE WHEN r.resets_at <= now() THEN excluded.hits
                    ELSE least(r.hits::bigint + excluded.hits, 2147483647)::integer END,
        resets_at = CASE WHEN r.resets_at <= now() THEN excluded.resets_at
                         ELSE r.resets_at END
    RETURNING r.hits, ceil(extract(epoch FROM r.resets_at - now()))::bigint;
    PERFORM set_config('campbooks.kernel_scope', coalesce(caller_scope, ''), true);
END
$$;

CREATE FUNCTION kernel.prune_rate_limits(batch integer)
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
          LIMIT greatest(coalesce(prune_rate_limits.batch, 0), 0));
    GET DIAGNOSTICS pruned = ROW_COUNT;
    PERFORM set_config('campbooks.kernel_scope', coalesce(caller_scope, ''), true);
    RETURN pruned;
END
$$;

REVOKE ALL ON FUNCTION kernel.take_rate_limit(bytea, double precision, integer) FROM PUBLIC;
REVOKE ALL ON FUNCTION kernel.prune_rate_limits(integer) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION kernel.take_rate_limit(bytea, double precision, integer)
    TO campbooks_app;
GRANT EXECUTE ON FUNCTION kernel.prune_rate_limits(integer) TO campbooks_app;
