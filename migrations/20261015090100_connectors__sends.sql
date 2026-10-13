-- Sends asked for by the conversations module (`conversations.send_requested`),
-- one row per send id: the idempotency key of the outbound message. The
-- outgoing message document is a blob the row holds while the send is open.
CREATE TABLE connectors.sends (
    -- The send id minted by conversations.
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    -- The messages stream of the inbox; NULL when it has none (or it was
    -- disconnected since).
    stream_id uuid REFERENCES connectors.streams (id) ON DELETE SET NULL,
    inbox_id uuid NOT NULL,
    -- The outgoing-message v1 document's blob reference.
    document jsonb NOT NULL,
    state text NOT NULL DEFAULT 'pending' CHECK (state IN ('pending', 'sent', 'failed')),
    -- Calls made to the provider; each is committed before the call, so a
    -- retry knows an earlier one may have gone through.
    attempts integer NOT NULL DEFAULT 0,
    attempted_at timestamptz,
    -- An attempt ended without a clear answer (a timeout, a 5xx).
    unclear boolean NOT NULL DEFAULT false,
    provider_message_id text,
    thread_key text,
    failure_reason text,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX sends_stream_idx ON connectors.sends (stream_id) WHERE stream_id IS NOT NULL;

ALTER TABLE connectors.sends ENABLE ROW LEVEL SECURITY;
ALTER TABLE connectors.sends FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON connectors.sends USING
    (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);

GRANT SELECT, INSERT, UPDATE, DELETE ON connectors.sends TO campbooks_app;
