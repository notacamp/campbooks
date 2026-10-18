SET LOCAL lock_timeout = '5s';

-- Who holds `campbooks_definer` (campbooks-cloud#213, items 8 and 12).
--
-- 1. The runtime role `campbooks_app` may not be a member of it. The role
--    owns the `SECURITY DEFINER` functions and holds the privileges their
--    statements need, so every runtime login (each a member of
--    `campbooks_app`) could then read the web sign-in's tables (sealed
--    tokens) and the rate-limit counters directly, outside the functions.
--    Which login the server signs in with is known only to the server, so
--    the server refuses, at every start, a role it queries as that holds
--    `campbooks_definer` in any way; this catches the grant to the runtime
--    role itself, which no installation needs.
-- 2. The migration login must be able to use the role's rights without
--    switching to it (`USAGE`: a membership that inherits), not only switch
--    to it (`SET`, checked when the role was created): a later migration
--    replacing a definer function (`CREATE OR REPLACE`) needs to act as its
--    owner. On PostgreSQL 16 and later a login that may grant the role to
--    itself does so with `INHERIT TRUE`; otherwise this fails, naming what
--    an administrator runs once.
DO $$
BEGIN
    IF pg_has_role('campbooks_app', 'campbooks_definer', 'MEMBER') THEN
        RAISE EXCEPTION 'the runtime role campbooks_app is a member of campbooks_definer, so '
                        'every runtime login could read the web sign-in''s tables directly: as '
                        'an administrator, run REVOKE campbooks_definer FROM campbooks_app; then '
                        'start Campbooks again'
            USING ERRCODE = '42501';
    END IF;

    IF NOT pg_has_role(current_user, 'campbooks_definer', 'USAGE') THEN
        IF current_setting('server_version_num')::int >= 160000 THEN
            BEGIN
                EXECUTE format('GRANT campbooks_definer TO %I WITH INHERIT TRUE', current_user);
            EXCEPTION WHEN insufficient_privilege OR invalid_grant_operation THEN
                NULL;
            END;
        END IF;
        IF NOT pg_has_role(current_user, 'campbooks_definer', 'USAGE') THEN
            RAISE EXCEPTION '% may switch to campbooks_definer but not use its rights, which a '
                            'migration replacing a definer function needs: as an administrator, '
                            'run once GRANT campbooks_definer TO % WITH INHERIT TRUE (PostgreSQL '
                            '16+; on 15, ALTER ROLE % INHERIT); then start Campbooks again',
                            current_user, quote_ident(current_user), quote_ident(current_user)
                USING ERRCODE = '42501';
        END IF;
    END IF;
END
$$;
