SET LOCAL lock_timeout = '5s';

-- `SECURITY DEFINER` functions run as a dedicated owner role, never as the
-- migration login (campbooks-cloud#214).
--
-- A definer function runs with its owner's rights. The kernel's two
-- rate-limit functions (and the BFF's, in the next migration) were owned by
-- the login that applied the migrations, which on the bundled Postgres is a
-- superuser: a bug in a function body would have run with every right in
-- the cluster. They now belong to `campbooks_definer`:
-- - NOLOGIN: nobody signs in as it, and only the functions run as it;
-- - no superuser, BYPASSRLS, CREATEDB, CREATEROLE, REPLICATION, and member
--   of no role (NOINHERIT besides);
-- - it owns nothing but the functions: no table, no schema;
-- - it holds exactly the privileges the functions' statements need, granted
--   by each schema's migration: here, using schema `kernel` and selecting,
--   inserting, updating and deleting rate-limit counters. The counters keep
--   row-level security forced (kernel scope only), which the functions turn
--   on for their own statement as before.
--
-- Roles belong to the whole cluster, so several Campbooks databases (or
-- test databases migrating in parallel) share it: create it only once, and
-- tolerate losing the race to another database's migration.
--
-- Who may run this: a superuser, or a login with CREATEROLE (the database
-- owner on most managed services). A login with neither needs the role
-- created once by an administrator, and the migration fails, saying so,
-- until it is (it does not leave the functions with the login, so every
-- installation ends in the same shape):
--   CREATE ROLE campbooks_definer NOLOGIN NOINHERIT;
--   GRANT campbooks_definer TO <migration login>;
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'campbooks_definer') THEN
        BEGIN
            CREATE ROLE campbooks_definer NOLOGIN NOSUPERUSER NOBYPASSRLS NOCREATEDB
                NOCREATEROLE NOREPLICATION NOINHERIT;
        EXCEPTION
            WHEN duplicate_object OR unique_violation THEN
                NULL;
            WHEN insufficient_privilege THEN
                RAISE EXCEPTION 'the role campbooks_definer does not exist and % may not create '
                                'it: as an administrator, run once CREATE ROLE campbooks_definer '
                                'NOLOGIN NOINHERIT; GRANT campbooks_definer TO %; then start '
                                'Campbooks again', current_user, quote_ident(current_user)
                    USING ERRCODE = '42501';
        END;
    END IF;
    -- A role created beforehand (by an administrator) must be as narrow.
    IF EXISTS (SELECT 1 FROM pg_roles
                WHERE rolname = 'campbooks_definer'
                  AND (rolcanlogin OR rolsuper OR rolbypassrls OR rolcreatedb OR rolcreaterole
                       OR rolreplication))
       OR EXISTS (SELECT 1 FROM pg_auth_members m JOIN pg_roles r ON r.oid = m.member
                   WHERE r.rolname = 'campbooks_definer') THEN
        RAISE EXCEPTION 'the role campbooks_definer may log in, has a privileged attribute or is '
                        'a member of another role: as an administrator, make it NOLOGIN '
                        'NOSUPERUSER NOBYPASSRLS NOCREATEDB NOCREATEROLE NOREPLICATION and revoke '
                        'its memberships; then start Campbooks again'
            USING ERRCODE = '42501';
    END IF;
    -- Handing a function over needs the login to be able to `SET ROLE` to
    -- its new owner, and replacing one later to be a member of it.
    -- (Superusers always are; a login with CREATEROLE that created the role
    -- may grant it to itself.)
    IF NOT pg_has_role(current_user, 'campbooks_definer', 'SET') THEN
        BEGIN
            GRANT campbooks_definer TO CURRENT_USER;
        EXCEPTION WHEN insufficient_privilege OR invalid_grant_operation THEN
            RAISE EXCEPTION '% may not take the role campbooks_definer: as an administrator, run '
                            'once GRANT campbooks_definer TO %; then start Campbooks again',
                            current_user, quote_ident(current_user)
                USING ERRCODE = '42501';
        END;
    END IF;
END
$$;

-- What the rate-limit functions' statements need, and nothing else.
GRANT USAGE ON SCHEMA kernel TO campbooks_definer;
GRANT SELECT, INSERT, UPDATE, DELETE ON kernel.rate_limits TO campbooks_definer;

-- The hand-over. A login that is not a superuser may give a function only to
-- a role allowed to create in its schema: that right is lent for the
-- hand-over and taken back in the same transaction.
GRANT CREATE ON SCHEMA kernel TO campbooks_definer;
ALTER FUNCTION kernel.take_rate_limit(bytea, double precision, integer)
    OWNER TO campbooks_definer;
ALTER FUNCTION kernel.prune_rate_limits(integer) OWNER TO campbooks_definer;
REVOKE CREATE ON SCHEMA kernel FROM campbooks_definer;

-- The grants survive a change of owner; asserted again all the same.
REVOKE ALL ON FUNCTION kernel.take_rate_limit(bytea, double precision, integer) FROM PUBLIC;
REVOKE ALL ON FUNCTION kernel.prune_rate_limits(integer) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION kernel.take_rate_limit(bytea, double precision, integer)
    TO campbooks_app;
GRANT EXECUTE ON FUNCTION kernel.prune_rate_limits(integer) TO campbooks_app;
