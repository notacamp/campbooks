SET LOCAL lock_timeout = '5s';

-- The web sign-in's tables are reachable only through the BFF's narrow
-- functions (campbooks-cloud#212).
--
-- `bff.sessions`, `bff.login_transactions` and `bff.retired_tokens` had no
-- row-level security and the runtime role could read and write every row
-- from any transaction: list or end anyone's web sessions, read pending
-- sign-ins, or remove entries from the retired-token list. Now the runtime
-- role holds no privilege on them. It calls the functions below, which run
-- as the tables' owner (`SECURITY DEFINER`, a pinned `search_path`, every
-- name qualified). Each one acts on the one session a cookie's token hash
-- names, the one sign-in a state hash names, or one account's sessions; only
-- the two prunes reach across accounts, and they delete ended rows only, a
-- bounded batch at a time.

-- 1. A started sign-in's nonce is sealed like its PKCE verifier (it was the
--    one plain-text value). In-flight sign-ins are deleted: they last ten
--    minutes, and one interrupted here answers `invalid_state` and starts
--    again. Sessions keep every row: their cookies are stored as SHA-256 and
--    their tokens sealed already.
DELETE FROM bff.login_transactions;
ALTER TABLE bff.login_transactions
    ALTER COLUMN nonce TYPE bytea USING convert_to(nonce, 'UTF8');

-- 2. No direct access. Row-level security with no policy (not forced) also
--    hides every row from any role given privileges later by mistake; the
--    owner, and so the functions, are unaffected.
REVOKE ALL ON bff.sessions, bff.login_transactions, bff.retired_tokens FROM campbooks_app;
REVOKE ALL ON bff.sessions, bff.login_transactions, bff.retired_tokens FROM PUBLIC;
ALTER TABLE bff.sessions ENABLE ROW LEVEL SECURITY;
ALTER TABLE bff.login_transactions ENABLE ROW LEVEL SECURITY;
ALTER TABLE bff.retired_tokens ENABLE ROW LEVEL SECURITY;

-- 3. Sessions. `presented` is always the SHA-256 of the token a request's
--    cookie carried; the database never sees a token.

-- A sign-in finished: the new session.
CREATE FUNCTION bff.create_session(new_id uuid, new_token bytea, account text,
                                   idp_session text, display_name text, display_email text,
                                   sealed_access bytea, access_until timestamptz,
                                   sealed_refresh bytea, sealed_id bytea, agent text,
                                   started timestamptz, idle_until timestamptz,
                                   ends timestamptz, asked text)
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
       OR NOT (started < ends AND idle_until <= ends) THEN
        RAISE EXCEPTION 'a session ends after it starts, idle at the latest when it ends'
            USING ERRCODE = '22023';
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

-- The live session whose current or previous token is `presented`;
-- `is_current` says which. Never its refresh or ID token.
CREATE FUNCTION bff.find_session(presented bytea)
    RETURNS TABLE (id uuid, is_current boolean, rotated_at timestamptz,
                   token_delivered boolean, subject text, name text, email text,
                   access_token bytea, access_expires_at timestamptz,
                   last_seen_at timestamptz, expires_at timestamptz, scopes text)
    LANGUAGE sql
    STABLE
    SECURITY DEFINER
    SET search_path = pg_catalog, pg_temp
AS $$
    SELECT s.id, s.token_hash = presented, s.rotated_at, s.token_delivered, s.subject,
           s.name, s.email, s.access_token, s.access_expires_at, s.last_seen_at,
           s.expires_at, s.scopes
      FROM bff.sessions s
     WHERE (s.token_hash = presented OR s.previous_token_hash = presented)
       AND s.idle_expires_at > now()
       AND s.expires_at > now()
$$;

-- Record use: the idle deadline moves, never past the absolute one.
CREATE FUNCTION bff.touch_session(presented bytea, seen timestamptz, idle_until timestamptz)
    RETURNS void
    LANGUAGE sql
    SECURITY DEFINER
    SET search_path = pg_catalog, pg_temp
AS $$
    UPDATE bff.sessions s
       SET last_seen_at = seen, idle_expires_at = least(idle_until, s.expires_at)
     WHERE (s.token_hash = presented OR s.previous_token_hash = presented)
       AND s.idle_expires_at > now()
       AND s.expires_at > now()
$$;

-- The current token `presented` reached the browser.
CREATE FUNCTION bff.mark_session_delivered(presented bytea)
    RETURNS void
    LANGUAGE sql
    SECURITY DEFINER
    SET search_path = pg_catalog, pg_temp
AS $$
    UPDATE bff.sessions s SET token_delivered = true WHERE s.token_hash = presented
$$;

-- Replace the current token `presented` by `fresh`, keeping `presented` as
-- the previous one (its grace) and retiring the one before it. False when
-- `presented` is no session's current token (another request rotated first).
CREATE FUNCTION bff.rotate_session(presented bytea, fresh bytea, rotated timestamptz)
    RETURNS boolean
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, pg_temp
AS $$
DECLARE
    found_id uuid;
    older bytea;
BEGIN
    IF fresh IS NULL OR octet_length(fresh) <> 32 OR rotated IS NULL THEN
        RAISE EXCEPTION 'a rotation needs a 32-byte token hash and a time' USING ERRCODE = '22023';
    END IF;
    SELECT s.id, s.previous_token_hash INTO found_id, older
      FROM bff.sessions s
     WHERE s.token_hash = presented
       FOR UPDATE;
    IF NOT FOUND THEN
        RETURN false;
    END IF;
    IF older IS NOT NULL THEN
        INSERT INTO bff.retired_tokens (token_hash, session_id, retired_at)
        VALUES (older, found_id, rotated)
        ON CONFLICT DO NOTHING;
    END IF;
    UPDATE bff.sessions s
       SET previous_token_hash = s.token_hash, token_hash = fresh, rotated_at = rotated,
           token_delivered = false
     WHERE s.id = found_id;
    RETURN true;
END
$$;

-- The browser still presents the previous token `presented` because the
-- current one never reached it: `fresh` replaces the undelivered one (which
-- is retired) once `presented` is past its grace (rotated at or before
-- `grace_since`). Answers `reissued`, `within_grace`, `delivered` or `gone`.
CREATE FUNCTION bff.reissue_session(presented bytea, fresh bytea, reissued timestamptz,
                                    grace_since timestamptz)
    RETURNS text
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, pg_temp
AS $$
DECLARE
    found_id uuid;
    undelivered bytea;
    was_delivered boolean;
    last_rotation timestamptz;
BEGIN
    IF fresh IS NULL OR octet_length(fresh) <> 32 OR reissued IS NULL OR grace_since IS NULL THEN
        RAISE EXCEPTION 'a reissue needs a 32-byte token hash and its times'
            USING ERRCODE = '22023';
    END IF;
    SELECT s.id, s.token_hash, s.token_delivered, s.rotated_at
      INTO found_id, undelivered, was_delivered, last_rotation
      FROM bff.sessions s
     WHERE s.previous_token_hash = presented
       FOR UPDATE;
    IF NOT FOUND THEN
        RETURN 'gone';
    END IF;
    IF was_delivered THEN
        RETURN 'delivered';
    END IF;
    IF last_rotation > grace_since THEN
        RETURN 'within_grace';
    END IF;
    INSERT INTO bff.retired_tokens (token_hash, session_id, retired_at)
    VALUES (undelivered, found_id, reissued)
    ON CONFLICT DO NOTHING;
    UPDATE bff.sessions s SET token_hash = fresh, rotated_at = reissued WHERE s.id = found_id;
    RETURN 'reissued';
END
$$;

-- The live session a retired token `presented` belonged to (a copied
-- cookie), or NULL.
CREATE FUNCTION bff.retired_session(presented bytea)
    RETURNS uuid
    LANGUAGE sql
    STABLE
    SECURITY DEFINER
    SET search_path = pg_catalog, pg_temp
AS $$
    SELECT s.id
      FROM bff.retired_tokens r
      JOIN bff.sessions s ON s.id = r.session_id
     WHERE r.token_hash = presented
       AND s.idle_expires_at > now()
       AND s.expires_at > now()
$$;

-- End the session that holds or held `presented` (current, previous or
-- retired): it is deleted, its retired tokens with it, and its sealed
-- refresh and ID tokens are answered for revoking.
CREATE FUNCTION bff.end_session(presented bytea)
    RETURNS TABLE (id uuid, refresh_token bytea, id_token bytea)
    LANGUAGE sql
    SECURITY DEFINER
    SET search_path = pg_catalog, pg_temp
AS $$
    DELETE FROM bff.sessions s
     WHERE s.token_hash = presented
        OR s.previous_token_hash = presented
        OR s.id = (SELECT r.session_id FROM bff.retired_tokens r WHERE r.token_hash = presented)
    RETURNING s.id, s.refresh_token, s.id_token
$$;

-- The tokens of the session `presented` names, locked until the caller's
-- transaction ends (the single-flight refresh).
CREATE FUNCTION bff.lock_session_tokens(presented bytea)
    RETURNS TABLE (access_token bytea, access_expires_at timestamptz, refresh_token bytea)
    LANGUAGE sql
    SECURITY DEFINER
    SET search_path = pg_catalog, pg_temp
AS $$
    SELECT s.access_token, s.access_expires_at, s.refresh_token
      FROM bff.sessions s
     WHERE s.token_hash = presented OR s.previous_token_hash = presented
       FOR UPDATE
$$;

-- Store the refreshed (sealed) tokens of the session `presented` names.
CREATE FUNCTION bff.store_session_tokens(presented bytea, sealed_access bytea,
                                         access_until timestamptz, sealed_refresh bytea)
    RETURNS void
    LANGUAGE sql
    SECURITY DEFINER
    SET search_path = pg_catalog, pg_temp
AS $$
    UPDATE bff.sessions s
       SET access_token = sealed_access, access_expires_at = access_until,
           refresh_token = sealed_refresh
     WHERE s.token_hash = presented OR s.previous_token_hash = presented
$$;

-- One account's sessions end: those of identity-provider session
-- `idp_session` (of `account` when both are given), or every session of
-- `account` (a back-channel logout, a sign-out everywhere, a suspension).
-- How many.
CREATE FUNCTION bff.end_account_sessions(account text, idp_session text)
    RETURNS bigint
    LANGUAGE sql
    SECURITY DEFINER
    SET search_path = pg_catalog, pg_temp
AS $$
    WITH ended AS (
        DELETE FROM bff.sessions s
         WHERE (idp_session IS NOT NULL AND s.idp_session_id = idp_session
                AND (account IS NULL OR s.subject = account))
            OR (idp_session IS NULL AND account IS NOT NULL AND s.subject = account)
        RETURNING 1)
    SELECT count(*) FROM ended
$$;

-- Delete up to `batch` (at most 10000) ended sessions; how many. The outer
-- condition is checked again on the row it deletes, so a session used (its
-- idle deadline moved) while the batch was chosen is kept.
CREATE FUNCTION bff.prune_sessions(batch integer)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, pg_temp
AS $$
DECLARE
    pruned bigint;
BEGIN
    DELETE FROM bff.sessions s
     WHERE s.id IN (
         SELECT e.id FROM bff.sessions e
          WHERE least(e.idle_expires_at, e.expires_at) <= now()
          LIMIT least(greatest(coalesce(batch, 0), 0), 10000))
       AND least(s.idle_expires_at, s.expires_at) <= now();
    GET DIAGNOSTICS pruned = ROW_COUNT;
    RETURN pruned;
END
$$;

-- 4. Sign-ins.

-- A sign-in started: `state` and `browser` are SHA-256 hashes, the nonce and
-- the PKCE verifier are sealed, `landing` is a same-origin path. At most an
-- hour long (the configured bound).
CREATE FUNCTION bff.start_login(state bytea, browser bytea, sealed_nonce bytea,
                                sealed_verifier bytea, landing text, started timestamptz,
                                ends timestamptz, asked text)
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
    IF landing IS NULL OR left(landing, 1) <> '/' THEN
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

-- Redeem the sign-in whose state hashes to `state`: deleted whatever happens
-- next, so a state works once.
CREATE FUNCTION bff.take_login(state bytea)
    RETURNS TABLE (state_hash bytea, browser_hash bytea, nonce bytea, code_verifier bytea,
                   return_to text, created_at timestamptz, expires_at timestamptz,
                   scopes text)
    LANGUAGE sql
    SECURITY DEFINER
    SET search_path = pg_catalog, pg_temp
AS $$
    DELETE FROM bff.login_transactions l
     WHERE l.state_hash = state
    RETURNING l.state_hash, l.browser_hash, l.nonce, l.code_verifier, l.return_to,
              l.created_at, l.expires_at, l.scopes
$$;

-- Delete up to `batch` (at most 10000) expired sign-ins; how many.
CREATE FUNCTION bff.prune_logins(batch integer)
    RETURNS bigint
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, pg_temp
AS $$
DECLARE
    pruned bigint;
BEGIN
    DELETE FROM bff.login_transactions l
     WHERE l.state_hash IN (
         SELECT e.state_hash FROM bff.login_transactions e
          WHERE e.expires_at <= now()
          LIMIT least(greatest(coalesce(batch, 0), 0), 10000))
       AND l.expires_at <= now();
    GET DIAGNOSTICS pruned = ROW_COUNT;
    RETURN pruned;
END
$$;

-- 5. The runtime role may call these, and nothing else may (functions are
--    executable by PUBLIC unless revoked).
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
