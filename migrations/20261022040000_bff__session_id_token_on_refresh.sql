SET LOCAL lock_timeout = '5s';

-- A session's ID token is stored again at a refresh (campbooks-cloud#191).
--
-- After a rotation of the instance secret key, what the previous key sealed
-- is sealed again with the current one as it is used. A web session's
-- access and refresh tokens are, at its next refresh; its ID token (the
-- proof the BFF shows the in-process issuer to end its session at sign-out)
-- was never stored again, so a session signed in before the rotation left
-- its issuer session behind at sign-out once the previous key was removed.
-- Now the refresh reads the ID token with the others and stores the
-- identity provider's new one, or the same one sealed with the current key
-- (`NULL` keeps the stored one).
--
-- The two functions change their result or arguments, so each is dropped
-- and created again. As before they belong to `campbooks_definer`, run with
-- a pinned path, and only the runtime role may call them. They are created
-- as their owner, which may create in the schema for this migration only,
-- so the migration login needs only to be able to switch to it (as the
-- migration that created the role required).
GRANT CREATE ON SCHEMA bff TO campbooks_definer;
SET LOCAL ROLE campbooks_definer;

DROP FUNCTION bff.lock_session_tokens(bytea);
-- The tokens of the session `presented` names, locked until the caller's
-- transaction ends (the single-flight refresh).
CREATE FUNCTION bff.lock_session_tokens(presented bytea)
    RETURNS TABLE (access_token bytea, access_expires_at timestamptz, refresh_token bytea,
                   id_token bytea)
    LANGUAGE sql
    SECURITY DEFINER
    SET search_path = pg_catalog, pg_temp
AS $$
    SELECT s.access_token, s.access_expires_at, s.refresh_token, s.id_token
      FROM bff.sessions s
     WHERE s.token_hash = presented OR s.previous_token_hash = presented
       FOR UPDATE
$$;

DROP FUNCTION bff.store_session_tokens(bytea, bytea, timestamptz, bytea);
-- Store the refreshed (sealed) tokens of the session `presented` names: a
-- refresh always keeps a refresh token (the new one, or the one it used),
-- and an ID token (`sealed_id`, or the stored one when it is `NULL`).
CREATE FUNCTION bff.store_session_tokens(presented bytea, sealed_access bytea,
                                         access_until timestamptz, sealed_refresh bytea,
                                         sealed_id bytea)
    RETURNS void
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = pg_catalog, pg_temp
AS $$
BEGIN
    IF sealed_access IS NULL OR octet_length(sealed_access) = 0
       OR sealed_refresh IS NULL OR octet_length(sealed_refresh) = 0
       OR (sealed_id IS NOT NULL AND octet_length(sealed_id) = 0)
       OR access_until IS NULL THEN
        RAISE EXCEPTION 'a refresh stores sealed tokens and a deadline' USING ERRCODE = '22023';
    END IF;
    UPDATE bff.sessions s
       SET access_token = sealed_access, access_expires_at = access_until,
           refresh_token = sealed_refresh, id_token = coalesce(sealed_id, s.id_token)
     WHERE s.token_hash = presented OR s.previous_token_hash = presented;
END
$$;

REVOKE ALL ON FUNCTION
    bff.lock_session_tokens(bytea),
    bff.store_session_tokens(bytea, bytea, timestamptz, bytea, bytea)
    FROM PUBLIC;
GRANT EXECUTE ON FUNCTION
    bff.lock_session_tokens(bytea),
    bff.store_session_tokens(bytea, bytea, timestamptz, bytea, bytea)
    TO campbooks_app;

RESET ROLE;
REVOKE CREATE ON SCHEMA bff FROM campbooks_definer;
