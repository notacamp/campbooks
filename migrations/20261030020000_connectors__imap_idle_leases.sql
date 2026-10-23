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

-- The keys the lease's foreign keys reference: a lease names its connection
-- with that connection's own workspace, and its stream with that stream's own
-- connection, so a row can never pair one workspace's connection with
-- another's stream.
ALTER TABLE connectors.connections
    ADD CONSTRAINT connections_id_workspace_key UNIQUE (id, workspace_id);
ALTER TABLE connectors.streams
    ADD CONSTRAINT streams_id_connection_key UNIQUE (id, connection_id);

CREATE TABLE connectors.imap_idle_leases (
    connection_id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    -- The connection's stream (an IMAP connection has one).
    stream_id uuid NOT NULL,
    -- Keyed digests of the mailbox's username at its host, and of the host:
    -- what the claim counts sessions by (one per login, a share per host).
    login_key text NOT NULL CHECK (login_key ~ '^[0-9a-f]{32}$'),
    host_key text NOT NULL CHECK (host_key ~ '^[0-9a-f]{32}$'),
    -- The host is one of the big providers Campbooks knows by name: its caps
    -- are a provider's, not a small server's.
    provider_host boolean NOT NULL,
    -- The last sync run offered it for IDLE; a withdrawn row (paused, needs
    -- the member, quiet, no IDLE, a new password) is kept, unheld, so its
    -- cooldown survives the next offer.
    offered boolean NOT NULL DEFAULT true,
    -- The holding process (a random id per start), until `expires_at`. A
    -- row without a holder is free from `expires_at` on (at once when null:
    -- a cooldown after a session gave up).
    holder uuid,
    expires_at timestamptz,
    -- When a claim last looked at it: claims read the longest unlooked-at
    -- rows first, so rows at their caps never hide the others.
    considered_at timestamptz,
    FOREIGN KEY (connection_id, workspace_id)
        REFERENCES connectors.connections (id, workspace_id) ON DELETE CASCADE,
    FOREIGN KEY (stream_id, connection_id)
        REFERENCES connectors.streams (id, connection_id) ON DELETE CASCADE,
    CHECK (holder IS NULL OR expires_at IS NOT NULL),
    CHECK (offered OR holder IS NULL)
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
-- in the maintenance scope only its holder and times change (never what was
-- offered, nor whether it is).
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
            OR NEW.provider_host IS DISTINCT FROM OLD.provider_host
            OR NEW.offered IS DISTINCT FROM OLD.offered) THEN
        RAISE EXCEPTION 'the maintenance scope changes only a lease''s holder and times'
            USING ERRCODE = 'check_violation';
    END IF;
    RETURN NEW;
END
$$;
CREATE TRIGGER imap_idle_leases_keep_owner BEFORE UPDATE ON connectors.imap_idle_leases
    FOR EACH ROW EXECUTE FUNCTION connectors.imap_idle_leases_keep_owner();

-- Pausing a stream, by whatever path, withdraws its connection's lease in the
-- same transaction (its holder's next renewal loses it): a paused mailbox is
-- never left signed in until the session's next refresh. The row is kept,
-- unheld, with its times. Runs as the caller, in the caller's workspace.
CREATE FUNCTION connectors.streams_paused_withdraw_idle() RETURNS trigger
    LANGUAGE plpgsql AS $$
BEGIN
    UPDATE connectors.imap_idle_leases SET offered = false, holder = NULL
     WHERE connection_id = NEW.connection_id AND offered;
    RETURN NULL;
END
$$;
CREATE TRIGGER streams_paused_withdraw_idle AFTER UPDATE OF paused ON connectors.streams
    FOR EACH ROW WHEN (NEW.paused AND NOT OLD.paused)
    EXECUTE FUNCTION connectors.streams_paused_withdraw_idle();

GRANT SELECT, INSERT, UPDATE, DELETE ON connectors.imap_idle_leases TO campbooks_app;
