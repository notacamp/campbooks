-- Instance apps (campbooks-cloud#369): confidential sign-in clients of the
-- whole instance (first-party single sign-on, such as Applicamp), registered
-- by an instance administrator from the server's CLI only
-- (`campbooks-server admin apps add|rotate-secret|remove`, on the migration
-- login). No HTTP endpoint registers, changes or removes one.
--
-- `identity.clients` gains the kind `instance`: it belongs to no workspace,
-- authenticates with a secret (stored as its SHA-256 only) and may name
-- where the browser goes after RP-initiated logout
-- (`post_logout_redirect_uris`, instance apps only).
--
-- The runtime role (every request) may read an instance app and record when
-- it last got a token, nothing else: a trigger refuses any other insert,
-- update or delete of an instance app by anyone who is not the table's owner
-- (the migration login the CLI runs on). Row-level security cannot say this
-- here: the table forces it on its owner too, and the owner is a member of
-- the runtime role.
SET LOCAL lock_timeout = '10s';

ALTER TABLE identity.clients ADD COLUMN post_logout_redirect_uris text[] NOT NULL DEFAULT '{}';

-- The checks that name the kinds, widened: the kind's own list, and "only a
-- workspace client has a secret" (now: a workspace or an instance client).
-- The second was declared unnamed; it is the one check over exactly `kind`
-- and `auth_method`.
ALTER TABLE identity.clients DROP CONSTRAINT clients_kind_check;
DO $$
DECLARE
    kind_col int2 := (SELECT attnum FROM pg_attribute
                       WHERE attrelid = 'identity.clients'::regclass AND attname = 'kind');
    method_col int2 := (SELECT attnum FROM pg_attribute
                         WHERE attrelid = 'identity.clients'::regclass AND attname = 'auth_method');
    found name;
    dropped int := 0;
BEGIN
    FOR found IN
        SELECT conname FROM pg_constraint
         WHERE conrelid = 'identity.clients'::regclass AND contype = 'c'
           AND conkey @> ARRAY[kind_col, method_col] AND cardinality(conkey) = 2
    LOOP
        EXECUTE format('ALTER TABLE identity.clients DROP CONSTRAINT %I', found);
        dropped := dropped + 1;
    END LOOP;
    IF dropped <> 1 THEN
        RAISE EXCEPTION 'expected one check over kind and auth_method on identity.clients, found %',
            dropped;
    END IF;
END $$;

ALTER TABLE identity.clients
    ADD CONSTRAINT clients_kind_check
        CHECK (kind IN ('workspace', 'dynamic', 'metadata_document', 'instance')),
    ADD CONSTRAINT clients_secret_kind_check
        CHECK (kind IN ('workspace', 'instance') OR auth_method = 'none'),
    ADD CONSTRAINT clients_instance_confidential_check
        CHECK (kind <> 'instance' OR (auth_method = 'client_secret_basic'
                                      AND cardinality(redirect_uris) > 0)),
    ADD CONSTRAINT clients_post_logout_instance_check
        CHECK (kind = 'instance' OR cardinality(post_logout_redirect_uris) = 0);

CREATE FUNCTION identity.instance_apps_from_the_cli() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF pg_catalog.pg_has_role(current_user,
                              (SELECT c.relowner FROM pg_catalog.pg_class c
                                WHERE c.oid = 'identity.clients'::regclass),
                              'USAGE') THEN
        RETURN coalesce(NEW, OLD);
    END IF;
    -- Recording that the app got a token is the runtime's.
    IF TG_OP = 'UPDATE'
       AND (NEW.client_id, NEW.kind, NEW.workspace_id, NEW.name, NEW.logo_uri,
            NEW.redirect_uris, NEW.grant_types, NEW.auth_method, NEW.secret_hash,
            NEW.allowed_scopes, NEW.created_by, NEW.created_at, NEW.secret_rotated_at,
            NEW.metadata_expires_at, NEW.post_logout_redirect_uris)
           IS NOT DISTINCT FROM
           (OLD.client_id, OLD.kind, OLD.workspace_id, OLD.name, OLD.logo_uri,
            OLD.redirect_uris, OLD.grant_types, OLD.auth_method, OLD.secret_hash,
            OLD.allowed_scopes, OLD.created_by, OLD.created_at, OLD.secret_rotated_at,
            OLD.metadata_expires_at, OLD.post_logout_redirect_uris) THEN
        RETURN NEW;
    END IF;
    RAISE EXCEPTION 'instance apps are registered, changed and removed only with campbooks-server admin apps'
        USING ERRCODE = 'insufficient_privilege';
END $$;

CREATE TRIGGER instance_apps_insert BEFORE INSERT ON identity.clients
    FOR EACH ROW WHEN (NEW.kind = 'instance')
    EXECUTE FUNCTION identity.instance_apps_from_the_cli();
CREATE TRIGGER instance_apps_update BEFORE UPDATE ON identity.clients
    FOR EACH ROW WHEN (OLD.kind = 'instance' OR NEW.kind = 'instance')
    EXECUTE FUNCTION identity.instance_apps_from_the_cli();
CREATE TRIGGER instance_apps_delete BEFORE DELETE ON identity.clients
    FOR EACH ROW WHEN (OLD.kind = 'instance')
    EXECUTE FUNCTION identity.instance_apps_from_the_cli();
