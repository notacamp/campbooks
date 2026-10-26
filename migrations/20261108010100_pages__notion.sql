-- Sync with Notion (pages-notion-sync, campbooks-cloud#376): a page can be
-- linked to a page in a member's Notion account and kept in step both ways.
-- The connectors module reads Notion and writes to it; this module owns what
-- the link means for a page. The connection and the Notion page are ids of
-- the connectors module and of Notion: no cross-schema key.
--
-- - `external_links`: the link, one per page. A Notion page is linked to one
--   page per connection at a time (a stopped link frees it).
-- - `external_blocks`: the three-way base. For each block that is the same
--   block at Notion, the hash of its content and the time Notion last edited
--   it as of the last sync, so a later change on either side, or on both, is
--   told apart. Hashes and times only: never any text.
-- - `notion_requests`: the block changes a member's edits asked for, bound to
--   the page, link and request with a MAC under a key the database never
--   holds (like the connectors module's mail and calendar requests), and
--   waiting for the connector. Their text is in a blob.
-- - `conflict_resolutions`: what a member chose for a conflict, with both
--   versions, so the choice can be undone. This is page content, kept with
--   the page and deleted with it.
--
-- Written to run again safely: every object is created only when missing.
--
-- Wait at most 5 s for a lock: a busy instance fails this migration fast,
-- and the next start retries it, rather than stalling.
SET LOCAL lock_timeout = '5s';

CREATE TABLE IF NOT EXISTS pages.external_links (
    page_id uuid PRIMARY KEY REFERENCES pages.pages (id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL,
    -- The connection (a connectors id) whose grant reads and writes Notion.
    connection_id uuid NOT NULL,
    -- The page at Notion.
    notion_page_id uuid NOT NULL,
    state text NOT NULL CHECK (state IN ('syncing', 'paused', 'stopped')),
    stopped_reason text CHECK (stopped_reason IN (
        'stopped_by_member', 'disconnected', 'trashed_here', 'trashed_in_notion',
        'not_shared', 'member_left', 'notion_unavailable')),
    -- The member whose Notion account it is: the one who imported it.
    created_by uuid NOT NULL,
    -- How many conflicts wait for the member.
    conflicts integer NOT NULL DEFAULT 0 CHECK (conflicts >= 0),
    last_inbound_at timestamptz,
    last_outbound_at timestamptz,
    -- The time of the newest snapshot applied: an older one never is.
    last_read_at timestamptz,
    -- When the page's body last changed on this side, and a push was asked for.
    push_requested_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    stopped_at timestamptz,
    CHECK ((state = 'stopped') = (stopped_reason IS NOT NULL))
);
CREATE UNIQUE INDEX IF NOT EXISTS external_links_notion_idx
    ON pages.external_links (connection_id, notion_page_id) WHERE state <> 'stopped';
CREATE INDEX IF NOT EXISTS external_links_connection_idx
    ON pages.external_links (workspace_id, connection_id) WHERE state <> 'stopped';
CREATE INDEX IF NOT EXISTS external_links_member_idx
    ON pages.external_links (workspace_id, created_by) WHERE state <> 'stopped';

CREATE TABLE IF NOT EXISTS pages.external_blocks (
    page_id uuid NOT NULL REFERENCES pages.pages (id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL,
    -- The block's stable id in the body (`bid`), and its id at Notion.
    bid uuid NOT NULL,
    notion_block_id uuid NOT NULL,
    -- What kind of block it is (`paragraph`, `page_link`, `external`, ...).
    kind text NOT NULL CHECK (char_length(kind) <= 32),
    -- SHA-256 of the block's normalised content at the last sync, and the
    -- time Notion last edited it as of then.
    base_hash bytea NOT NULL CHECK (octet_length(base_hash) = 32),
    base_edited_at timestamptz NOT NULL,
    PRIMARY KEY (page_id, bid),
    UNIQUE (page_id, notion_block_id)
);

CREATE TABLE IF NOT EXISTS pages.notion_requests (
    id uuid PRIMARY KEY,
    page_id uuid NOT NULL REFERENCES pages.pages (id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL,
    connection_id uuid NOT NULL,
    notion_page_id uuid NOT NULL,
    -- The changes, in order: ids, kinds, hashes and times (no text).
    items jsonb NOT NULL,
    -- The blob reference of the document with what to write.
    body jsonb NOT NULL,
    mac bytea,
    state text NOT NULL DEFAULT 'waiting' CHECK (state IN ('waiting', 'done')),
    created_by uuid NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    done_at timestamptz
);
CREATE INDEX IF NOT EXISTS notion_requests_waiting_idx
    ON pages.notion_requests (page_id) WHERE state = 'waiting';

CREATE TABLE IF NOT EXISTS pages.conflict_resolutions (
    id uuid PRIMARY KEY,
    page_id uuid NOT NULL REFERENCES pages.pages (id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL,
    bid uuid NOT NULL,
    choice text NOT NULL CHECK (choice IN ('mine', 'notion', 'both')),
    resolved_by uuid NOT NULL,
    -- Both versions as the conflict showed them (normalised blocks), and the
    -- base before the choice moved it.
    ours jsonb NOT NULL,
    theirs jsonb NOT NULL,
    base_hash bytea CHECK (octet_length(base_hash) = 32),
    base_edited_at timestamptz,
    undone boolean NOT NULL DEFAULT false,
    at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS conflict_resolutions_page_idx
    ON pages.conflict_resolutions (page_id, at);

DO $$
DECLARE
    t text;
BEGIN
    FOREACH t IN ARRAY ARRAY['external_links', 'external_blocks', 'notion_requests',
                             'conflict_resolutions']
    LOOP
        EXECUTE format('ALTER TABLE pages.%I ENABLE ROW LEVEL SECURITY', t);
        EXECUTE format('ALTER TABLE pages.%I FORCE ROW LEVEL SECURITY', t);
        EXECUTE format('DROP POLICY IF EXISTS workspace_isolation ON pages.%I', t);
        EXECUTE format(
            'CREATE POLICY workspace_isolation ON pages.%I USING '
            '(workspace_id = nullif(current_setting(''campbooks.workspace_id'', true), '''')::uuid)',
            t);
        EXECUTE format('GRANT SELECT, INSERT, UPDATE, DELETE ON pages.%I TO campbooks_app', t);
    END LOOP;
END
$$;
