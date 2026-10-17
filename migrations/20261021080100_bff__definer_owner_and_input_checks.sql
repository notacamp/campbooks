SET LOCAL lock_timeout = '5s';

-- The BFF's `SECURITY DEFINER` functions run as `campbooks_definer` (created
-- by the previous migration), never as the migration login, and check their
-- input more strictly (campbooks-cloud#214).

-- 1. Stricter input. Same names, arguments, results, owner and grants; only
--    the bodies change (and two become `plpgsql` to check before acting).
--    Replaced before the hand-over below, while the login still owns them.

-- A sign-in finished: the new session, at most 366 days long (the
-- configured absolute timeout's bound).
CREATE OR REPLACE FUNCTION bff.create_session(new_id uuid, new_token bytea, account text,
                                              idp_session text, display_name text,
                                              display_email text, sealed_access bytea,
                                              access_until timestamptz, sealed_refresh bytea,
                                              sealed_id bytea, agent text, started timestamptz,
                                              idle_until timestamptz, ends timestamptz,
                                              asked text)
    RETURNS void
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, pg_temp
AS $$
BEGIN
    IF new_id IS NULL OR new_token IS NULL OR octet_length(new_token) <> 32 THEN
        RAISE EXCEPTION 'a session has an id and a 32-byte token hash' USING ERRCODE = '22023';
    END IF;
    IF account IS NULL OR btrim(account) = '' THEN
        RAISE EXCEPTION 'a session belongs to an account' USING ERRCODE = '22023';
    END IF;
    IF sealed_access IS NULL OR sealed_id IS NULL OR access_until IS NULL THEN
        RAISE EXCEPTION 'a session holds sealed tokens' USING ERRCODE = '22023';
    END IF;
    IF started IS NULL OR idle_until IS NULL OR ends IS NULL
       OR NOT (started < ends AND started < idle_until AND idle_until <= ends) THEN
        RAISE EXCEPTION 'a session ends after it starts, idle at the latest when it ends'
            USING ERRCODE = '22023';
    END IF;
    IF ends - started > interval '366 days' THEN
        RAISE EXCEPTION 'a session lasts at most 366 days' USING ERRCODE = '22023';
    END IF;
    INSERT INTO bff.sessions
        (id, token_hash, rotated_at, subject, idp_session_id, name, email,
         access_token, access_expires_at, refresh_token, id_token, user_agent,
         created_at, last_seen_at, idle_expires_at, expires_at, scopes)
    VALUES (new_id, new_token, started, account, idp_session, display_name, display_email,
            sealed_access, access_until, sealed_refresh, sealed_id, agent,
            started, started, idle_until, ends, asked);
END
$$;

-- Record use: the idle deadline moves, never past the absolute one. Both
-- times are required, the deadline after the use (a missing deadline would
-- otherwise keep the session until its absolute end).
CREATE OR REPLACE FUNCTION bff.touch_session(presented bytea, seen timestamptz,
                                             idle_until timestamptz)
    RETURNS void
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, pg_temp
AS $$
BEGIN
    IF seen IS NULL OR idle_until IS NULL OR NOT seen < idle_until THEN
        RAISE EXCEPTION 'a use has a time and a later idle deadline' USING ERRCODE = '22023';
    END IF;
    UPDATE bff.sessions s
       SET last_seen_at = seen, idle_expires_at = least(idle_until, s.expires_at)
     WHERE (s.token_hash = presented OR s.previous_token_hash = presented)
       AND s.idle_expires_at > now()
       AND s.expires_at > now();
END
$$;

-- Store the refreshed (sealed) tokens of the session `presented` names: a
-- refresh always keeps a refresh token (the new one, or the one it used).
CREATE OR REPLACE FUNCTION bff.store_session_tokens(presented bytea, sealed_access bytea,
                                                    access_until timestamptz,
                                                    sealed_refresh bytea)
    RETURNS void
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, pg_temp
AS $$
BEGIN
    IF sealed_access IS NULL OR octet_length(sealed_access) = 0
       OR sealed_refresh IS NULL OR octet_length(sealed_refresh) = 0
       OR access_until IS NULL THEN
        RAISE EXCEPTION 'a refresh stores sealed tokens and a deadline' USING ERRCODE = '22023';
    END IF;
    UPDATE bff.sessions s
       SET access_token = sealed_access, access_expires_at = access_until,
           refresh_token = sealed_refresh
     WHERE s.token_hash = presented OR s.previous_token_hash = presented;
END
$$;

-- A sign-in started. `landing` is a same-origin path, checked as the
-- handler checks it: at most 2 048 characters, starting with one `/` and
-- never `//` (another host to a browser), only visible ASCII (no space or
-- control character, which browsers drop), no backslash (which browsers
-- read as `/`), none of `"`, `'`, `<`, `>` and the backtick, and not the
-- BFF's own paths.
CREATE OR REPLACE FUNCTION bff.start_login(state bytea, browser bytea, sealed_nonce bytea,
                                           sealed_verifier bytea, landing text,
                                           started timestamptz, ends timestamptz, asked text)
    RETURNS void
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, pg_temp
AS $$
BEGIN
    IF state IS NULL OR octet_length(state) <> 32
       OR browser IS NULL OR octet_length(browser) <> 32 THEN
        RAISE EXCEPTION 'a sign-in has 32-byte state and browser hashes' USING ERRCODE = '22023';
    END IF;
    IF sealed_nonce IS NULL OR sealed_verifier IS NULL THEN
        RAISE EXCEPTION 'a sign-in holds a sealed nonce and verifier' USING ERRCODE = '22023';
    END IF;
    IF landing IS NULL OR length(landing) > 2048
       OR left(landing, 1) <> '/' OR left(landing, 2) = '//'
       OR landing !~ '^[!-~]+$'
       OR strpos(landing, chr(92)) > 0 OR landing ~ '["''<>`]'
       OR landing = '/bff' OR left(landing, 5) IN ('/bff/', '/bff?', '/bff#') THEN
        RAISE EXCEPTION 'a sign-in lands on a same-origin path' USING ERRCODE = '22023';
    END IF;
    IF started IS NULL OR ends IS NULL
       OR NOT (started < ends AND ends - started <= interval '1 hour') THEN
        RAISE EXCEPTION 'a sign-in lasts more than nothing and at most an hour'
            USING ERRCODE = '22023';
    END IF;
    INSERT INTO bff.login_transactions
        (state_hash, browser_hash, nonce, code_verifier, return_to, created_at, expires_at,
         scopes)
    VALUES (state, browser, sealed_nonce, sealed_verifier, landing, started, ends, asked);
END
$$;

-- 2. What the functions' statements need, and nothing else: no `UPDATE` of
--    a sign-in, no `UPDATE` or `DELETE` of a retired token (they go with
--    their session: the foreign key's cascade runs as the table's owner).
GRANT USAGE ON SCHEMA bff TO campbooks_definer;
GRANT SELECT, INSERT, UPDATE, DELETE ON bff.sessions TO campbooks_definer;
GRANT SELECT, INSERT, DELETE ON bff.login_transactions TO campbooks_definer;
GRANT SELECT, INSERT ON bff.retired_tokens TO campbooks_definer;

-- 3. Row-level security stays enabled (not forced) with one policy, for the
--    functions' owner only: the tables keep their owner, and every other
--    role, the runtime role included, still sees no row even if granted a
--    privilege by mistake.
CREATE POLICY definer_functions ON bff.sessions TO campbooks_definer
    USING (true) WITH CHECK (true);
CREATE POLICY definer_functions ON bff.login_transactions TO campbooks_definer
    USING (true) WITH CHECK (true);
CREATE POLICY definer_functions ON bff.retired_tokens TO campbooks_definer
    USING (true) WITH CHECK (true);

-- 4. The hand-over (the right to create in the schema is lent for it, as in
--    the kernel's migration).
GRANT CREATE ON SCHEMA bff TO campbooks_definer;
ALTER FUNCTION bff.create_session(uuid, bytea, text, text, text, text, bytea, timestamptz, bytea,
                                  bytea, text, timestamptz, timestamptz, timestamptz, text)
    OWNER TO campbooks_definer;
ALTER FUNCTION bff.find_session(bytea) OWNER TO campbooks_definer;
ALTER FUNCTION bff.touch_session(bytea, timestamptz, timestamptz) OWNER TO campbooks_definer;
ALTER FUNCTION bff.mark_session_delivered(bytea) OWNER TO campbooks_definer;
ALTER FUNCTION bff.rotate_session(bytea, bytea, timestamptz) OWNER TO campbooks_definer;
ALTER FUNCTION bff.reissue_session(bytea, bytea, timestamptz, timestamptz)
    OWNER TO campbooks_definer;
ALTER FUNCTION bff.retired_session(bytea) OWNER TO campbooks_definer;
ALTER FUNCTION bff.end_session(bytea) OWNER TO campbooks_definer;
ALTER FUNCTION bff.lock_session_tokens(bytea) OWNER TO campbooks_definer;
ALTER FUNCTION bff.store_session_tokens(bytea, bytea, timestamptz, bytea)
    OWNER TO campbooks_definer;
ALTER FUNCTION bff.end_account_sessions(text, text) OWNER TO campbooks_definer;
ALTER FUNCTION bff.prune_sessions(integer) OWNER TO campbooks_definer;
ALTER FUNCTION bff.start_login(bytea, bytea, bytea, bytea, text, timestamptz, timestamptz, text)
    OWNER TO campbooks_definer;
ALTER FUNCTION bff.take_login(bytea) OWNER TO campbooks_definer;
ALTER FUNCTION bff.prune_logins(integer) OWNER TO campbooks_definer;
REVOKE CREATE ON SCHEMA bff FROM campbooks_definer;

-- 5. The runtime role may call them, and nothing else may (the grants
--    survive the change of owner; asserted again all the same).
REVOKE ALL ON FUNCTION
    bff.create_session(uuid, bytea, text, text, text, text, bytea, timestamptz, bytea, bytea,
                       text, timestamptz, timestamptz, timestamptz, text),
    bff.find_session(bytea),
    bff.touch_session(bytea, timestamptz, timestamptz),
    bff.mark_session_delivered(bytea),
    bff.rotate_session(bytea, bytea, timestamptz),
    bff.reissue_session(bytea, bytea, timestamptz, timestamptz),
    bff.retired_session(bytea),
    bff.end_session(bytea),
    bff.lock_session_tokens(bytea),
    bff.store_session_tokens(bytea, bytea, timestamptz, bytea),
    bff.end_account_sessions(text, text),
    bff.prune_sessions(integer),
    bff.start_login(bytea, bytea, bytea, bytea, text, timestamptz, timestamptz, text),
    bff.take_login(bytea),
    bff.prune_logins(integer)
    FROM PUBLIC;
GRANT EXECUTE ON FUNCTION
    bff.create_session(uuid, bytea, text, text, text, text, bytea, timestamptz, bytea, bytea,
                       text, timestamptz, timestamptz, timestamptz, text),
    bff.find_session(bytea),
    bff.touch_session(bytea, timestamptz, timestamptz),
    bff.mark_session_delivered(bytea),
    bff.rotate_session(bytea, bytea, timestamptz),
    bff.reissue_session(bytea, bytea, timestamptz, timestamptz),
    bff.retired_session(bytea),
    bff.end_session(bytea),
    bff.lock_session_tokens(bytea),
    bff.store_session_tokens(bytea, bytea, timestamptz, bytea),
    bff.end_account_sessions(text, text),
    bff.prune_sessions(integer),
    bff.start_login(bytea, bytea, bytea, bytea, text, timestamptz, timestamptz, text),
    bff.take_login(bytea),
    bff.prune_logins(integer)
    TO campbooks_app;
