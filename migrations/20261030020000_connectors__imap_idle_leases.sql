-- IMAP push with IDLE (imap-idle, campbooks-cloud#318): which IMAP
-- connections may be held in IDLE, and which process holds each. A process
-- that runs sync keeps a pool of IDLE sessions; a lease row gives each
-- connection one holder at a time. No message content and no address here:
-- ids, two keyed digests (the mailbox's login and its host, HMAC-SHA256
-- under the instance secret), a flag and times.
--
-- Wait at most 5 s for a lock: a busy instance fails this migration fast,
-- and the next start retries it, rather than stalling.
SET LOCAL lock_timeout = '5s';

CREATE TABLE connectors.imap_idle_leases (
    connection_id uuid PRIMARY KEY REFERENCES connectors.connections (id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL,
    -- The connection's stream (an IMAP connection has one).
    stream_id uuid NOT NULL REFERENCES connectors.streams (id) ON DELETE CASCADE,
    -- Keyed digests of the mailbox's username at its host, and of the host:
    -- what the claim counts sessions by (one per login, a share per host).
    login_key text NOT NULL CHECK (login_key ~ '^[0-9a-f]{32}$'),
    host_key text NOT NULL CHECK (host_key ~ '^[0-9a-f]{32}$'),
    -- The host is a known provider's (a fixed IMAP host of the bundled
    -- provider database): its caps are a provider's, not a small server's.
    provider_host boolean NOT NULL,
    -- The holding process (a random id per start), until `expires_at`. A
    -- row without a holder is free from `expires_at` on (at once when null:
    -- a cooldown after a session gave up).
    holder uuid,
    expires_at timestamptz,
    claimed_at timestamptz,
    CHECK (holder IS NULL OR expires_at IS NOT NULL)
);

CREATE INDEX imap_idle_leases_holder_idx ON connectors.imap_idle_leases (holder)
    WHERE holder IS NOT NULL;
CREATE INDEX imap_idle_leases_host_idx ON connectors.imap_idle_leases (host_key)
    WHERE holder IS NOT NULL;

ALTER TABLE connectors.imap_idle_leases ENABLE ROW LEVEL SECURITY;
ALTER TABLE connectors.imap_idle_leases FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON connectors.imap_idle_leases USING
    (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);

-- The pool claims, renews and releases leases across workspaces in the
-- maintenance scope (`kernel::db::begin_maintenance`, which
-- `architecture.toml` lets the pool's lease file call), like the stream
-- dispatcher on `connectors.sync_due`: it reads (ids, keys, times) and
-- updates, never inserts or deletes. A sync run offers and withdraws a row
-- in its own workspace.
CREATE POLICY maintenance_reads_idle_leases ON connectors.imap_idle_leases
    FOR SELECT
    USING (current_setting('campbooks.maintenance_scope', true) = 'on');
CREATE POLICY maintenance_claims_idle_leases ON connectors.imap_idle_leases
    FOR UPDATE
    USING (current_setting('campbooks.maintenance_scope', true) = 'on')
    WITH CHECK (current_setting('campbooks.maintenance_scope', true) = 'on');

-- A row never moves to another connection or workspace, whoever updates it;
-- in the maintenance scope only its holder and times change.
CREATE FUNCTION connectors.imap_idle_leases_keep_owner() RETURNS trigger
    LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.connection_id IS DISTINCT FROM OLD.connection_id
       OR NEW.workspace_id IS DISTINCT FROM OLD.workspace_id THEN
        RAISE EXCEPTION 'imap_idle_leases rows keep their connection and workspace'
            USING ERRCODE = 'check_violation';
    END IF;
    IF current_setting('campbooks.maintenance_scope', true) = 'on'
       AND (NEW.stream_id IS DISTINCT FROM OLD.stream_id
            OR NEW.login_key IS DISTINCT FROM OLD.login_key
            OR NEW.host_key IS DISTINCT FROM OLD.host_key
            OR NEW.provider_host IS DISTINCT FROM OLD.provider_host) THEN
        RAISE EXCEPTION 'the maintenance scope changes only a lease''s holder and times'
            USING ERRCODE = 'check_violation';
    END IF;
    RETURN NEW;
END
$$;
CREATE TRIGGER imap_idle_leases_keep_owner BEFORE UPDATE ON connectors.imap_idle_leases
    FOR EACH ROW EXECUTE FUNCTION connectors.imap_idle_leases_keep_owner();

GRANT SELECT, INSERT, UPDATE, DELETE ON connectors.imap_idle_leases TO campbooks_app;
