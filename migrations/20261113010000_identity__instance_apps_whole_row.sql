-- Instance apps (campbooks-cloud#383): the guard on `identity.clients` now
-- compares whole rows. The first version listed the columns by hand, so a
-- column added to the table later and missed in that list would have been
-- writable on an instance app by the runtime role. The runtime may still
-- record when an app last got a token (`last_used_at`); any other difference
-- between the old and the new row is refused, whatever columns the table has.
-- Insert and delete are as they were, and so is who counts as the CLI: the
-- table's owner (the migration login).
SET LOCAL lock_timeout = '10s';

CREATE OR REPLACE FUNCTION identity.instance_apps_from_the_cli() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF pg_catalog.pg_has_role(current_user,
                              (SELECT c.relowner FROM pg_catalog.pg_class c
                                WHERE c.oid = 'identity.clients'::regclass),
                              'USAGE') THEN
        RETURN coalesce(NEW, OLD);
    END IF;
    -- Recording that the app got a token is the runtime's.
    IF TG_OP = 'UPDATE'
       AND pg_catalog.to_jsonb(NEW) - 'last_used_at' = pg_catalog.to_jsonb(OLD) - 'last_used_at' THEN
        RETURN NEW;
    END IF;
    RAISE EXCEPTION 'instance apps are registered, changed and removed only with campbooks-server admin apps'
        USING ERRCODE = 'insufficient_privilege';
END $$;
