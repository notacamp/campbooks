-- The `pages` module owns the `pages` schema and nothing else: what members
-- write for themselves and their team (campbooks-cloud#363). Other modules'
-- records are referenced by type and id only (no cross-schema joins or
-- foreign keys); the members and the workspace are ids of the workspaces
-- module.
CREATE SCHEMA IF NOT EXISTS pages;

CREATE TABLE pages.pages (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    -- NULL for a top-level page. A page whose parent is purged for good
    -- (when the page was trashed on its own earlier) becomes top-level.
    parent_id uuid REFERENCES pages.pages (id) ON DELETE SET NULL,
    -- `private` (its creator's) or `workspace` (every member's). Kept on
    -- every row; a sub-page always has its parent's.
    place text NOT NULL CHECK (place IN ('private', 'workspace')),
    -- A fractional-indexing key, ordered among the siblings.
    position text NOT NULL CHECK (position <> '' AND char_length(position) <= 64),
    -- A projection set by `pages.update`: 0 to 200 characters. Never in an
    -- event.
    title text NOT NULL DEFAULT '' CHECK (char_length(title) <= 200),
    -- One of a fixed set of icon names.
    icon text CHECK (icon IS NULL OR char_length(icon) <= 32),
    created_by uuid NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    edited_by uuid NOT NULL,
    edited_at timestamptz NOT NULL DEFAULT now(),
    trashed_at timestamptz,
    trashed_by uuid,
    -- The page whose deletion trashed this one (itself for a root).
    trash_root uuid,
    -- Grows with every change; carried by events.
    revision bigint NOT NULL DEFAULT 1,
    -- Grows with a change of the title, icon, place or position:
    -- what `pages.update` and `pages.move` check ("stale" is 409).
    meta_revision bigint NOT NULL DEFAULT 1,
    -- When the last `body` change was announced (at most one every 5 s).
    last_body_event_at timestamptz,
    CHECK (parent_id IS DISTINCT FROM id),
    CHECK ((trashed_at IS NULL) = (trash_root IS NULL))
);
CREATE INDEX pages_siblings_idx ON pages.pages (workspace_id, parent_id, position);
CREATE INDEX pages_trash_root_idx ON pages.pages (workspace_id, trash_root)
    WHERE trash_root IS NOT NULL;
CREATE INDEX pages_trashed_idx ON pages.pages (trashed_at) WHERE trash_root = id;
CREATE INDEX pages_creator_idx ON pages.pages (workspace_id, created_by);

-- The body as Yjs updates (v1), append-only between compactions.
CREATE TABLE pages.updates (
    page_id uuid NOT NULL REFERENCES pages.pages (id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL,
    seq bigint NOT NULL,
    update bytea NOT NULL,
    author uuid NOT NULL,
    at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (page_id, seq)
);

-- The body so far: a full Yjs update, covering updates up to `upto_seq`.
CREATE TABLE pages.snapshots (
    page_id uuid PRIMARY KEY REFERENCES pages.pages (id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL,
    state bytea NOT NULL,
    upto_seq bigint NOT NULL,
    at timestamptz NOT NULL DEFAULT now()
);

-- Who a page is shared with, and as what. Applies to the page and to every
-- sub-page that has no grant of its own for that member.
CREATE TABLE pages.grants (
    page_id uuid NOT NULL REFERENCES pages.pages (id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL,
    member_id uuid NOT NULL,
    role text NOT NULL CHECK (role IN ('reader', 'writer', 'manager')),
    granted_by uuid NOT NULL,
    at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (page_id, member_id)
);
CREATE INDEX grants_member_idx ON pages.grants (workspace_id, member_id);

-- What a page mentions, embeds or links to: a projection rebuilt from the
-- body on every accepted update. References only, never a copy of what they
-- point at.
CREATE TABLE pages.links (
    page_id uuid NOT NULL REFERENCES pages.pages (id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL,
    ref_type text NOT NULL,
    ref_id text NOT NULL CHECK (char_length(ref_id) <= 320),
    kind text NOT NULL CHECK (kind IN ('mention', 'embed', 'child')),
    PRIMARY KEY (page_id, ref_type, ref_id, kind)
);
CREATE INDEX links_target_idx ON pages.links (workspace_id, ref_type, ref_id);

-- Files and images a page holds: the blob (by its hash) and who uploaded it,
-- so a block may only name a blob uploaded to this page or by the caller.
-- The file's name is page content and lives in the body only.
CREATE TABLE pages.files (
    page_id uuid NOT NULL REFERENCES pages.pages (id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL,
    hash text NOT NULL CHECK (hash ~ '^[0-9a-f]{64}$'),
    size bigint NOT NULL,
    content_type text NOT NULL,
    uploaded_by uuid NOT NULL,
    at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (page_id, hash)
);
CREATE INDEX files_uploader_idx ON pages.files (workspace_id, uploaded_by, hash);

DO $$
DECLARE
    t text;
BEGIN
    FOREACH t IN ARRAY ARRAY['pages', 'updates', 'snapshots', 'grants', 'links', 'files']
    LOOP
        EXECUTE format('ALTER TABLE pages.%I ENABLE ROW LEVEL SECURITY', t);
        EXECUTE format('ALTER TABLE pages.%I FORCE ROW LEVEL SECURITY', t);
        EXECUTE format(
            'CREATE POLICY workspace_isolation ON pages.%I USING '
            '(workspace_id = nullif(current_setting(''campbooks.workspace_id'', true), '''')::uuid)',
            t);
    END LOOP;
END
$$;

-- The daily trash purge's dispatcher (`pages.dispatch_purge_trash`) reads,
-- in the maintenance scope, which workspaces hold a trash root older than 30
-- days: read only, and it reads nothing but ids and times.
CREATE POLICY maintenance_reads_trash ON pages.pages FOR SELECT
    USING (current_setting('campbooks.maintenance_scope', true) = 'on' AND trash_root = id);

GRANT USAGE ON SCHEMA pages TO campbooks_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA pages TO campbooks_app;
