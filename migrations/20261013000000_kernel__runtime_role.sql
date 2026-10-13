-- The runtime role (ADR 0011, `campbooks_kernel::db`). The server's pool
-- switches every connection to it (`SET ROLE`), so Postgres row-level
-- security applies to every request and job even when the login user is a
-- superuser (superusers and BYPASSRLS roles skip policies). Migrations keep
-- running as the login user. NOLOGIN: nobody signs in as it.
--
-- Roles belong to the whole cluster, so several Campbooks databases (or
-- test databases migrating in parallel) share it: create it only once, and
-- tolerate losing the race to another database's migration. A login user
-- without CREATEROLE needs the role created once by an administrator:
--   CREATE ROLE campbooks_app NOLOGIN; GRANT campbooks_app TO <login user>;
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'campbooks_app') THEN
        BEGIN
            CREATE ROLE campbooks_app NOLOGIN NOSUPERUSER NOBYPASSRLS NOCREATEDB NOCREATEROLE;
        EXCEPTION WHEN duplicate_object OR unique_violation THEN
            NULL;
        END;
    END IF;
    -- The login user must be allowed to `SET ROLE campbooks_app`.
    -- (Superusers always are.)
    IF NOT pg_has_role(current_user, 'campbooks_app', 'SET') THEN
        EXECUTE format('GRANT campbooks_app TO %I', current_user);
    END IF;
END
$$;

-- Each migration grants the runtime role what its own tables need. The
-- kernel's tables carry no workspace policies: the queue and the event log
-- are infrastructure, read and written only through the kernel's API.
GRANT USAGE ON SCHEMA kernel TO campbooks_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON kernel.events, kernel.jobs, kernel.schedules
    TO campbooks_app;
GRANT EXECUTE ON FUNCTION kernel.uuid_v7() TO campbooks_app;
