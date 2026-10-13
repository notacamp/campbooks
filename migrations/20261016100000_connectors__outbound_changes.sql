-- Outbound changes (gmail-writeback): a member's change, recorded from
-- `conversations.changes_requested`, waiting to be made at the provider and
-- then kept as the record of what happened to it. Ids, label ids and stable
-- reason codes only: never message content.
CREATE TABLE connectors.outbound_changes (
    -- UUIDv7: the order changes were recorded in, which is the order they
    -- are applied in.
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    stream_id uuid NOT NULL REFERENCES connectors.streams (id) ON DELETE CASCADE,
    -- The `conversations.changes_requested` event it came from.
    request_id uuid NOT NULL,
    provider_message_id text NOT NULL,
    kind text NOT NULL CHECK (kind IN ('mark_read', 'mark_unread', 'star', 'unstar', 'archive',
                                       'unarchive', 'add_label', 'remove_label',
                                       'move_to_stream', 'unsupported')),
    -- The provider labels to put on and take off.
    add_labels text[] NOT NULL DEFAULT '{}',
    remove_labels text[] NOT NULL DEFAULT '{}',
    -- The provider's history id when the change was recorded: where the
    -- change sits in the provider's order of writes (last writer wins).
    stamp_history_id bigint,
    state text NOT NULL DEFAULT 'pending'
        CHECK (state IN ('pending', 'applied', 'superseded', 'skipped', 'failed')),
    -- Why it ended as it did (`replaced`, `changed_at_provider`,
    -- `message_gone`, `label_not_at_provider`, `refused`, …).
    reason text,
    -- Runs that tried and hit provider trouble.
    attempts integer NOT NULL DEFAULT 0,
    -- The provider history ids around the run that made it: the provider's
    -- record of this write falls in (applied_from, applied_to].
    applied_from bigint,
    applied_to bigint,
    created_at timestamptz NOT NULL DEFAULT now(),
    done_at timestamptz,
    UNIQUE (request_id, provider_message_id)
);
CREATE INDEX outbound_changes_pending_idx ON connectors.outbound_changes (stream_id, id)
    WHERE state = 'pending';
CREATE INDEX outbound_changes_message_idx
    ON connectors.outbound_changes (stream_id, provider_message_id);
CREATE INDEX outbound_changes_done_idx ON connectors.outbound_changes (done_at)
    WHERE state <> 'pending';

-- A message whose state Campbooks must be told again (a member's change was
-- overtaken at the provider or refused by it): the next run re-emits it.
ALTER TABLE connectors.gmail_messages ADD COLUMN reemit boolean NOT NULL DEFAULT false;
CREATE INDEX gmail_messages_reemit_idx ON connectors.gmail_messages (stream_id) WHERE reemit;

ALTER TABLE connectors.outbound_changes ENABLE ROW LEVEL SECURITY;
ALTER TABLE connectors.outbound_changes FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON connectors.outbound_changes USING
    (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);

GRANT SELECT, INSERT, UPDATE, DELETE ON connectors.outbound_changes TO campbooks_app;
