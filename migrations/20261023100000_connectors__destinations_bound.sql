-- Sends and outbound changes bound to where they go (campbooks-cloud#242).
--
-- A send's MACs (`conversations.drafts.request_mac`, `send_mac`) and an
-- outbound change's (`connectors.outbound_changes.mac`) now name the
-- connection, stream and mailbox they go out through, and an outbound
-- change that put a message in the trash also binds where to put it back
-- (`restore_to`, Zoho). Those are new encodings (`…/v2`); nothing in this
-- migration changes a MAC.
--
-- What was bound before this deploy names no destination. It still
-- verifies, but only while its destination is still the one recorded here,
-- in tables the runtime role may read but never write:
--
-- - `connectors.destinations_at_deploy`: each messages stream's inbox,
--   connection and mailbox, as they are now;
-- - `connectors.restores_at_deploy`: each outbound change's `restore_to`,
--   as it is now.
--
-- Nothing is added to them after this migration: a row the runtime role
-- inserts is refused, so SQL cannot pin a destination of its choosing. A
-- stream or a change that goes away takes its row with it. See the change's
-- design.md.
SET LOCAL lock_timeout = '10s';

CREATE TABLE connectors.destinations_at_deploy (
    stream_id uuid PRIMARY KEY REFERENCES connectors.streams (id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL,
    inbox_id uuid NOT NULL,
    connection_id uuid NOT NULL,
    mailbox_key text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX destinations_at_deploy_workspace_idx
    ON connectors.destinations_at_deploy (workspace_id);

CREATE TABLE connectors.restores_at_deploy (
    change_id uuid PRIMARY KEY REFERENCES connectors.outbound_changes (id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL,
    restore_to text[] NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX restores_at_deploy_workspace_idx
    ON connectors.restores_at_deploy (workspace_id);

ALTER TABLE connectors.destinations_at_deploy ENABLE ROW LEVEL SECURITY;
ALTER TABLE connectors.destinations_at_deploy FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON connectors.destinations_at_deploy
    USING (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);

ALTER TABLE connectors.restores_at_deploy ENABLE ROW LEVEL SECURITY;
ALTER TABLE connectors.restores_at_deploy FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON connectors.restores_at_deploy
    USING (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);

-- The snapshots. The tables force row-level security even for their owner,
-- and no workspace is set here: it is lifted for these statements only,
-- inside the migration's transaction.
ALTER TABLE connectors.streams NO FORCE ROW LEVEL SECURITY;
ALTER TABLE connectors.outbound_changes NO FORCE ROW LEVEL SECURITY;
ALTER TABLE connectors.destinations_at_deploy NO FORCE ROW LEVEL SECURITY;
ALTER TABLE connectors.restores_at_deploy NO FORCE ROW LEVEL SECURITY;
INSERT INTO connectors.destinations_at_deploy
    (stream_id, workspace_id, inbox_id, connection_id, mailbox_key)
SELECT s.id, s.workspace_id, s.inbox_id, s.connection_id, s.mailbox_key
  FROM connectors.streams s
 WHERE s.capability = 'messages' AND s.inbox_id IS NOT NULL;
INSERT INTO connectors.restores_at_deploy (change_id, workspace_id, restore_to)
SELECT o.id, o.workspace_id, o.restore_to
  FROM connectors.outbound_changes o
 WHERE o.restore_to IS NOT NULL;
ALTER TABLE connectors.restores_at_deploy FORCE ROW LEVEL SECURITY;
ALTER TABLE connectors.destinations_at_deploy FORCE ROW LEVEL SECURITY;
ALTER TABLE connectors.outbound_changes FORCE ROW LEVEL SECURITY;
ALTER TABLE connectors.streams FORCE ROW LEVEL SECURITY;

-- The runtime role reads the snapshots (its workspace's rows) and never
-- writes them.
GRANT SELECT ON connectors.destinations_at_deploy TO campbooks_app;
GRANT SELECT ON connectors.restores_at_deploy TO campbooks_app;
