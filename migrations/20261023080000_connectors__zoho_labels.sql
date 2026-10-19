-- Zoho labels and stream marks (provider-stream-labels, campbooks-cloud#292).
--
-- The Zoho adapter now reads a message's labels and writes them back, and
-- carries a stream Zoho has no place for as a label named `Campbooks/<Stream>`
-- (a stream mark). This caches the names of a Zoho mail account's labels, as
-- `gmail_labels` does Gmail's: refreshed at each sweep and when a listing
-- names a label it does not know, and when the adapter creates a stream mark.
-- Ids and names only.
--
-- Wait at most 5 s for a lock: a busy instance fails this migration fast,
-- and the next start retries it, rather than stalling.
SET LOCAL lock_timeout = '5s';

CREATE TABLE connectors.zoho_labels (
    workspace_id uuid NOT NULL,
    stream_id uuid NOT NULL REFERENCES connectors.streams (id) ON DELETE CASCADE,
    label_id text NOT NULL,
    name text NOT NULL,
    refreshed_at timestamptz NOT NULL,
    PRIMARY KEY (stream_id, label_id)
);

ALTER TABLE connectors.zoho_labels ENABLE ROW LEVEL SECURITY;
ALTER TABLE connectors.zoho_labels FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON connectors.zoho_labels USING
    (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);

GRANT SELECT, INSERT, UPDATE, DELETE ON connectors.zoho_labels TO campbooks_app;
