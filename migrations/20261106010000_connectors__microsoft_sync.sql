-- Reading Microsoft mailboxes (microsoft-mail-sync, campbooks-cloud#316): a
-- Microsoft stream keeps each folder's Graph delta position in its cursor,
-- so its reading needs no table. Graph's change notifications do: one row
-- per connection with its subscription, which the public callback
-- (`POST /hooks/microsoft/mail`) finds across workspaces to know whose
-- stream to run. No message content and no address here: ids, the keyed hash
-- of the subscription's `clientState` (HMAC-SHA256 under the instance key;
-- the secret itself is never stored) and times. The row goes with its
-- connection.
--
-- Written to run again safely: every object is created only when missing.
--
-- Wait at most 5 s for a lock: a busy instance fails this migration fast,
-- and the next start retries it, rather than stalling.
SET LOCAL lock_timeout = '5s';

CREATE TABLE IF NOT EXISTS connectors.microsoft_subscriptions (
    connection_id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    -- The connection's messages stream: what a notification runs.
    stream_id uuid NOT NULL,
    -- Graph's id; none after a creation Graph refused (`retry_at` set).
    subscription_id text UNIQUE CHECK (subscription_id ~ '^[A-Za-z0-9-]{1,64}$'),
    -- HMAC-SHA256 of the `clientState`, 32 bytes.
    client_state_tag bytea CHECK (octet_length(client_state_tag) = 32),
    expires_at timestamptz,
    -- After a refused creation, the next try is not before this.
    retry_at timestamptz,
    updated_at timestamptz NOT NULL,
    FOREIGN KEY (connection_id, workspace_id)
        REFERENCES connectors.connections (id, workspace_id) ON DELETE CASCADE,
    FOREIGN KEY (stream_id, connection_id)
        REFERENCES connectors.streams (id, connection_id) ON DELETE CASCADE,
    CHECK ((subscription_id IS NULL) = (client_state_tag IS NULL)),
    CHECK ((subscription_id IS NULL) = (expires_at IS NULL)),
    CHECK (subscription_id IS NOT NULL OR retry_at IS NOT NULL)
);

ALTER TABLE connectors.microsoft_subscriptions ENABLE ROW LEVEL SECURITY;
ALTER TABLE connectors.microsoft_subscriptions FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS workspace_isolation ON connectors.microsoft_subscriptions;
CREATE POLICY workspace_isolation ON connectors.microsoft_subscriptions USING
    (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);

-- The callback finds a notification's subscription across workspaces in the
-- maintenance scope (`kernel::db::begin_maintenance`, which `architecture.toml`
-- lets the callback's file call): it reads (ids, the hash, the workspace) and
-- nothing else. Whatever it then does with a connection (its stream's run, a
-- lifecycle notice) happens in that connection's workspace.
DROP POLICY IF EXISTS maintenance_reads_microsoft_subscriptions
    ON connectors.microsoft_subscriptions;
CREATE POLICY maintenance_reads_microsoft_subscriptions ON connectors.microsoft_subscriptions
    FOR SELECT
    USING (current_setting('campbooks.maintenance_scope', true) = 'on');

GRANT SELECT, INSERT, UPDATE, DELETE ON connectors.microsoft_subscriptions TO campbooks_app;
