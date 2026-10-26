-- Sync with Notion (pages-notion-sync, campbooks-cloud#376): a fifth
-- provider, Notion, with the `pages` capability. A Notion connection lives in
-- `connectors.connections` with its tokens sealed in `connectors.credentials`
-- like any other (the provider's `bot_id` is its `account_subject`). Each
-- linked Notion page is one change stream (capability `pages`, `mailbox_key`
-- the Notion page id), so the engine's schedule, budgets, health and pauses
-- apply unchanged.
--
-- What is new here:
--
-- - `notion_authorizations`: a consent started and not finished, single use,
--   bound to the browser that goes to Notion and back (a cookie only that
--   browser holds; only hashes are kept) and valid for ten minutes. Notion
--   returns no ID token, so this module runs the authorization-code flow
--   itself. The PKCE verifier is sealed with the credentials key.
-- - `notion_links`: which Notion page each `pages` stream reads, and the
--   pages module's page it feeds (an id only). The public webhook finds the
--   streams of a Notion page across workspaces in the maintenance scope
--   (read only: ids), and only makes them due.
-- - `notion_changes`: the block changes the pages module asked for, recorded
--   once it confirmed them, bound to the request with a MAC under a key the
--   database never holds, and made by the stream's next run before it reads.
--   Ids, kinds, times and reason codes only: what is written travels in the
--   request's blob, which the pages module stored.
-- - `notion_webhook_token`: the verification token Notion posts once to
--   `POST /hooks/notion` when the operator creates a webhook subscription,
--   while no webhook secret is configured. One row for the instance, sealed
--   with the instance key; the operator reads it with
--   `campbooks-server admin notion-webhook-token` and pastes it into
--   Notion, where it becomes the signing secret. The runtime role may
--   replace it (delete and insert) but never read it back: no request, event
--   or log ever carries it.
--
-- Written to run again safely: every object is created only when missing.
--
-- Wait at most 5 s for a lock: a busy instance fails this migration fast,
-- and the next start retries it, rather than stalling.
SET LOCAL lock_timeout = '5s';

ALTER TABLE connectors.connections DROP CONSTRAINT IF EXISTS connections_provider_check;
ALTER TABLE connectors.connections
    ADD CONSTRAINT connections_provider_check
    CHECK (provider IN ('google', 'zoho', 'imap', 'microsoft', 'notion'));

ALTER TABLE connectors.connections DROP CONSTRAINT IF EXISTS connections_capabilities_check;
ALTER TABLE connectors.connections
    ADD CONSTRAINT connections_capabilities_check
    CHECK (capabilities <@ ARRAY['messages', 'calendar', 'files', 'bank', 'pages']);

ALTER TABLE connectors.streams DROP CONSTRAINT IF EXISTS streams_capability_check;
ALTER TABLE connectors.streams
    ADD CONSTRAINT streams_capability_check
    CHECK (capability IN ('messages', 'calendar', 'files', 'bank', 'pages'));

CREATE TABLE IF NOT EXISTS connectors.notion_authorizations (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    -- The member who started it, and the only one who can finish it.
    user_id uuid NOT NULL,
    -- A connection of theirs to connect again.
    connection_id uuid REFERENCES connectors.connections (id) ON DELETE CASCADE,
    -- A path of this instance to send the browser back to.
    return_to text CHECK (return_to IS NULL OR char_length(return_to) <= 256),
    -- SHA-256 of the `state` sent to Notion; set when the browser goes there.
    state_hash bytea UNIQUE CHECK (octet_length(state_hash) = 32),
    -- SHA-256 of the cookie that binds the round trip to its browser.
    browser_hash bytea CHECK (octet_length(browser_hash) = 32),
    -- The PKCE verifier, sealed with the credentials key (bound to this row).
    pkce_verifier bytea,
    expires_at timestamptz NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    CHECK ((state_hash IS NULL) = (browser_hash IS NULL)),
    CHECK ((state_hash IS NULL) = (pkce_verifier IS NULL))
);
CREATE INDEX IF NOT EXISTS notion_authorizations_expires_idx
    ON connectors.notion_authorizations (expires_at);

CREATE TABLE IF NOT EXISTS connectors.notion_links (
    stream_id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    connection_id uuid NOT NULL,
    -- The Notion page the stream reads.
    notion_page_id uuid NOT NULL,
    -- The pages module's page it feeds (an id of that module).
    page_id uuid NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (connection_id, notion_page_id),
    FOREIGN KEY (connection_id, workspace_id)
        REFERENCES connectors.connections (id, workspace_id) ON DELETE CASCADE,
    FOREIGN KEY (stream_id, connection_id)
        REFERENCES connectors.streams (id, connection_id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS notion_links_page_idx ON connectors.notion_links (notion_page_id);

CREATE TABLE IF NOT EXISTS connectors.notion_changes (
    -- UUIDv7: the order changes were recorded in, which is the order they
    -- are made in.
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    stream_id uuid NOT NULL REFERENCES connectors.streams (id) ON DELETE CASCADE,
    -- The pages module's request, the page it is for and the item's place in it.
    request_id uuid NOT NULL,
    page_id uuid NOT NULL,
    seq integer NOT NULL CHECK (seq >= 0),
    kind text NOT NULL CHECK (kind IN ('update', 'append', 'delete')),
    -- The block in the pages module.
    block_id uuid NOT NULL,
    -- The block at Notion (update, delete).
    notion_block_id uuid,
    -- Where an appended block goes: under this block or page at Notion, or
    -- under a block of the same request once it has been made (none of
    -- the two: under the page); after this block at Notion, or after a
    -- block of the same request once it has been made (none: at the end).
    parent_id uuid,
    parent_block_id uuid,
    after_notion_id uuid,
    after_block_id uuid,
    -- When the block was last edited at Notion as of the last sync: a block
    -- edited since is not written.
    base_edited_at timestamptz,
    -- The blob reference of the request's document, stored and verified by
    -- the pages module: what is written is that document and no other.
    body jsonb NOT NULL,
    mac bytea,
    state text NOT NULL DEFAULT 'pending'
        CHECK (state IN ('pending', 'applied', 'skipped', 'failed')),
    -- Why it ended as it did (a stable code, never Notion's words).
    reason text,
    attempts integer NOT NULL DEFAULT 0,
    -- What Notion answered (applied): the block's id and edit time.
    result_notion_id uuid,
    result_edited_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    done_at timestamptz,
    UNIQUE (request_id, seq)
);
CREATE INDEX IF NOT EXISTS notion_changes_pending_idx
    ON connectors.notion_changes (stream_id, id) WHERE state = 'pending';
CREATE INDEX IF NOT EXISTS notion_changes_done_idx
    ON connectors.notion_changes (done_at) WHERE state <> 'pending';

DO $$
DECLARE
    t text;
BEGIN
    FOREACH t IN ARRAY ARRAY['notion_authorizations', 'notion_links', 'notion_changes']
    LOOP
        EXECUTE format('ALTER TABLE connectors.%I ENABLE ROW LEVEL SECURITY', t);
        EXECUTE format('ALTER TABLE connectors.%I FORCE ROW LEVEL SECURITY', t);
        EXECUTE format('DROP POLICY IF EXISTS workspace_isolation ON connectors.%I', t);
        EXECUTE format(
            'CREATE POLICY workspace_isolation ON connectors.%I USING '
            '(workspace_id = nullif(current_setting(''campbooks.workspace_id'', true), '''')::uuid)',
            t);
    END LOOP;
END
$$;

-- The webhook finds a Notion page's streams across workspaces in the
-- maintenance scope (`kernel::db::begin_maintenance`, which
-- `architecture.toml` lets the hook's file call): it reads ids and nothing
-- else. Whatever it then does with a stream (make it due) happens in that
-- stream's workspace.
DROP POLICY IF EXISTS maintenance_reads_notion_links ON connectors.notion_links;
CREATE POLICY maintenance_reads_notion_links ON connectors.notion_links
    FOR SELECT
    USING (current_setting('campbooks.maintenance_scope', true) = 'on');

CREATE TABLE IF NOT EXISTS connectors.notion_webhook_token (
    -- The one row of the instance.
    only_row boolean PRIMARY KEY DEFAULT true CHECK (only_row),
    -- The token, sealed with the instance key (bound to this table).
    token bytea NOT NULL,
    received_at timestamptz NOT NULL DEFAULT now()
);

GRANT SELECT, INSERT, UPDATE, DELETE ON connectors.notion_authorizations TO campbooks_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON connectors.notion_links TO campbooks_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON connectors.notion_changes TO campbooks_app;

-- Instance-level, with no workspace data: the runtime role can only replace
-- the row, never read it (the CLI, on the migration login, does).
GRANT INSERT, DELETE ON connectors.notion_webhook_token TO campbooks_app;
