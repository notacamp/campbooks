-- The `connectors` module (ADR 0010): connections to outside accounts, their
-- encrypted credentials, the sync engine's streams and the Gmail adapter's
-- index. It owns the `connectors` schema and nothing else; users and inboxes
-- of other modules are referenced by id only.
CREATE SCHEMA IF NOT EXISTS connectors;

-- A member's authorisation for Campbooks to use one outside account. Belongs
-- to the workspace; `granted_by` is the member whose consent it is.
CREATE TABLE connectors.connections (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    provider text NOT NULL CHECK (provider IN ('google')),
    -- The provider's stable id of the account (OpenID `sub`).
    account_subject text NOT NULL,
    -- The account's email address, lower-cased, and display name.
    address text NOT NULL,
    display_name text,
    granted_by uuid NOT NULL,
    capabilities text[] NOT NULL DEFAULT '{}'
        CHECK (capabilities <@ ARRAY['messages', 'calendar', 'files', 'bank']),
    -- Space-separated scopes the provider granted.
    scopes text NOT NULL DEFAULT '',
    health text NOT NULL DEFAULT 'ok'
        CHECK (health IN ('ok', 'needs_reauth', 'throttled', 'failing')),
    health_reason text,
    health_changed_at timestamptz NOT NULL DEFAULT now(),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, provider, account_subject)
);
CREATE INDEX connections_granted_by_idx ON connectors.connections (workspace_id, granted_by);

-- Secrets, sealed with AES-256-GCM (kernel::crypto, purpose
-- `campbooks connectors credentials v1`; associated data names table,
-- column and row). Never selected by a read operation.
CREATE TABLE connectors.credentials (
    connection_id uuid PRIMARY KEY REFERENCES connectors.connections (id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL,
    refresh_token bytea NOT NULL,
    access_token bytea,
    access_expires_at timestamptz,
    updated_at timestamptz NOT NULL DEFAULT now()
);

-- A started consent, completed by the member who started it. Single use;
-- expires after ten minutes. Only the SHA-256 of `state` is kept.
CREATE TABLE connectors.authorizations (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    user_id uuid NOT NULL,
    provider text NOT NULL CHECK (provider IN ('google')),
    capabilities text[] NOT NULL,
    connection_id uuid REFERENCES connectors.connections (id) ON DELETE CASCADE,
    state_hash bytea NOT NULL UNIQUE,
    pkce_verifier bytea NOT NULL,
    expires_at timestamptz NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX authorizations_expires_idx ON connectors.authorizations (expires_at);

-- One change stream per connection and capability, with its cursor.
CREATE TABLE connectors.streams (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    connection_id uuid NOT NULL REFERENCES connectors.connections (id) ON DELETE CASCADE,
    capability text NOT NULL CHECK (capability IN ('messages', 'calendar', 'files', 'bank')),
    -- The conversations inbox this stream feeds (minted here; `messages`).
    inbox_id uuid,
    phase text NOT NULL CHECK (phase IN ('backfill', 'incremental', 'resync')),
    -- Adapter-specific and opaque elsewhere (Gmail: history id, page token,
    -- window start, resync generation).
    cursor jsonb NOT NULL DEFAULT '{}',
    interval_seconds integer NOT NULL CHECK (interval_seconds > 0),
    paused boolean NOT NULL DEFAULT false,
    last_synced_at timestamptz,
    last_change_at timestamptz,
    consecutive_failures integer NOT NULL DEFAULT 0,
    throttled_until timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (connection_id, capability)
);

-- The scheduling index the one-minute dispatcher reads across workspaces:
-- ids and times only. Its rows are visible to the stream's workspace, and to
-- maintenance transactions that set no workspace (the dispatcher).
CREATE TABLE connectors.sync_due (
    stream_id uuid PRIMARY KEY REFERENCES connectors.streams (id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL,
    due_at timestamptz NOT NULL
);
CREATE INDEX sync_due_at_idx ON connectors.sync_due (due_at);

-- The Gmail adapter's index of what it has emitted: lets a label-only change
-- be re-emitted without downloading the message, and a resync find what is
-- gone. `document` is the normalised message (without flags) as a blob.
CREATE TABLE connectors.gmail_messages (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    stream_id uuid NOT NULL REFERENCES connectors.streams (id) ON DELETE CASCADE,
    provider_message_id text NOT NULL,
    thread_id text NOT NULL,
    label_ids text[] NOT NULL DEFAULT '{}',
    internal_date timestamptz NOT NULL,
    document_hash text NOT NULL,
    document_size bigint NOT NULL,
    -- The resync pass that last listed it.
    listed_generation integer NOT NULL DEFAULT 0,
    UNIQUE (stream_id, provider_message_id)
);
CREATE INDEX gmail_messages_window_idx ON connectors.gmail_messages (stream_id, internal_date);

-- Gmail label names per stream (`labels.list`), for user labels.
CREATE TABLE connectors.gmail_labels (
    workspace_id uuid NOT NULL,
    stream_id uuid NOT NULL REFERENCES connectors.streams (id) ON DELETE CASCADE,
    label_id text NOT NULL,
    name text NOT NULL,
    kind text NOT NULL,
    PRIMARY KEY (stream_id, label_id)
);

-- The producer's hold on each batch blob (contracts/README.md), released
-- after the retention period.
CREATE TABLE connectors.batches (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    stream_id uuid NOT NULL REFERENCES connectors.streams (id) ON DELETE CASCADE,
    batch_hash text NOT NULL,
    batch_size bigint NOT NULL,
    cursor text NOT NULL,
    changes integer NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX batches_created_idx ON connectors.batches (created_at);

-- A token bucket per provider, shared by every process: instance-level, no
-- workspace data.
CREATE TABLE connectors.provider_budgets (
    provider text PRIMARY KEY,
    tokens double precision NOT NULL,
    refilled_at timestamptz NOT NULL
);

-- Row-level security (ADR 0011) on every workspace-owned table.
DO $$
DECLARE t text;
BEGIN
    FOREACH t IN ARRAY ARRAY['connections', 'credentials', 'authorizations', 'streams',
                             'sync_due', 'gmail_messages', 'gmail_labels', 'batches']
    LOOP
        EXECUTE format('ALTER TABLE connectors.%I ENABLE ROW LEVEL SECURITY', t);
        EXECUTE format('ALTER TABLE connectors.%I FORCE ROW LEVEL SECURITY', t);
        EXECUTE format(
            'CREATE POLICY workspace_isolation ON connectors.%I USING '
            '(workspace_id = nullif(current_setting(''campbooks.workspace_id'', true), '''')::uuid)',
            t);
    END LOOP;
END $$;

-- The dispatcher's maintenance transaction sets no workspace and must see
-- every stream's due time (ids and times only).
CREATE POLICY maintenance_reads_schedule ON connectors.sync_due
    USING (nullif(current_setting('campbooks.workspace_id', true), '') IS NULL);

GRANT USAGE ON SCHEMA connectors TO campbooks_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA connectors TO campbooks_app;
