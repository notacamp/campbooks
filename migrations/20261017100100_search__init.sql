-- The `search` module (ADR 0016): a projection of the records other modules
-- own, kept current from their events through the kernel's `SearchSource`
-- port. Other modules' records are referenced by type and id only.
--
-- Every table is workspace-owned and isolated by forced row-level security
-- (ADR 0011). Nothing here is the source of truth: dropping every row and
-- running the backfills rebuilds it.
CREATE SCHEMA IF NOT EXISTS search;

-- Text search configurations: one per shipped language plus a neutral one,
-- each ignoring accents (`unaccent`, installed by the kernel) before
-- stemming. An entry is indexed in its own language; a query term is looked
-- up in all of them.
CREATE TEXT SEARCH CONFIGURATION search.simple (COPY = pg_catalog.simple);
ALTER TEXT SEARCH CONFIGURATION search.simple
    ALTER MAPPING FOR hword, hword_part, word WITH unaccent, simple;
CREATE TEXT SEARCH CONFIGURATION search.en (COPY = pg_catalog.english);
ALTER TEXT SEARCH CONFIGURATION search.en
    ALTER MAPPING FOR hword, hword_part, word WITH unaccent, english_stem;
CREATE TEXT SEARCH CONFIGURATION search.pt (COPY = pg_catalog.portuguese);
ALTER TEXT SEARCH CONFIGURATION search.pt
    ALTER MAPPING FOR hword, hword_part, word WITH unaccent, portuguese_stem;
CREATE TEXT SEARCH CONFIGURATION search.es (COPY = pg_catalog.spanish);
ALTER TEXT SEARCH CONFIGURATION search.es
    ALTER MAPPING FOR hword, hword_part, word WITH unaccent, spanish_stem;
CREATE TEXT SEARCH CONFIGURATION search.fr (COPY = pg_catalog.french);
ALTER TEXT SEARCH CONFIGURATION search.fr
    ALTER MAPPING FOR hword, hword_part, word WITH unaccent, french_stem;

-- The searchable vector of an entry: its group's title (weight A), its
-- people and attachment names (B, unstemmed) and its text (C), the title and
-- text in the entry's language.
CREATE FUNCTION search.entry_vector(title text, people text, files text, body text, language text)
RETURNS tsvector LANGUAGE sql STABLE PARALLEL SAFE AS $$
    SELECT setweight(to_tsvector(('search.' || language)::regconfig, title), 'A')
        || setweight(to_tsvector('search.simple'::regconfig, people), 'B')
        || setweight(to_tsvector('search.simple'::regconfig, files), 'B')
        || setweight(to_tsvector(('search.' || language)::regconfig, body), 'C')
$$;

-- What a result is shown as, and what access applies to (a conversation).
CREATE TABLE search.groups (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    record_type text NOT NULL,
    record_id text NOT NULL,
    -- The container whose visibility decides who sees it (an inbox).
    container_type text NOT NULL,
    container_id text NOT NULL,
    occurred_at timestamptz NOT NULL,
    title text NOT NULL DEFAULT '',
    -- The title lower-cased and without accents, for `subject:`.
    title_folded text NOT NULL DEFAULT '',
    -- Names that are not text in it (label names), folded, for `label:`.
    keywords text[] NOT NULL DEFAULT '{}',
    -- Non-personal values filters test (`kernel::search::facet`).
    facets jsonb NOT NULL DEFAULT '{}',
    -- The shape it was indexed with (`domain::index::INDEX_VERSION`).
    index_version smallint NOT NULL,
    refreshed_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, record_type, record_id)
);
CREATE INDEX groups_list_idx
    ON search.groups (workspace_id, container_id, occurred_at DESC, id DESC);
CREATE INDEX groups_title_trgm_idx ON search.groups USING gin (title_folded gin_trgm_ops);

-- The text inside a group (a message).
CREATE TABLE search.entries (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    group_id uuid NOT NULL REFERENCES search.groups (id) ON DELETE CASCADE,
    record_type text NOT NULL,
    record_id text NOT NULL,
    occurred_at timestamptz NOT NULL,
    language text NOT NULL CHECK (language IN ('en', 'pt', 'es', 'fr', 'simple')),
    -- `[{role, name?, address}]`.
    people jsonb NOT NULL DEFAULT '[]',
    -- Names and addresses (split into words) and attachment names, as
    -- indexed.
    people_words text NOT NULL DEFAULT '',
    file_words text NOT NULL DEFAULT '',
    -- "name address" per role, lower-cased and without accents, for
    -- `from:`, `to:` and `cc:`.
    from_folded text NOT NULL DEFAULT '',
    to_folded text NOT NULL DEFAULT '',
    cc_folded text NOT NULL DEFAULT '',
    -- The plain text the source gave (bounded, quoted history removed).
    body text NOT NULL DEFAULT '',
    attachments text[] NOT NULL DEFAULT '{}',
    facets jsonb NOT NULL DEFAULT '{}',
    -- The source's content stamp: text is read again only when it changes.
    stamp text NOT NULL,
    tsv tsvector NOT NULL,
    UNIQUE (workspace_id, record_type, record_id)
);
CREATE INDEX entries_tsv_idx ON search.entries USING gin (tsv);
CREATE INDEX entries_group_idx ON search.entries (group_id);
CREATE INDEX entries_from_trgm_idx ON search.entries USING gin (from_folded gin_trgm_ops);
CREATE INDEX entries_to_trgm_idx ON search.entries USING gin (to_folded gin_trgm_ops);
CREATE INDEX entries_cc_trgm_idx ON search.entries USING gin (cc_folded gin_trgm_ops);

-- One index build per workspace and source: started lazily, resumable.
CREATE TABLE search.backfills (
    workspace_id uuid NOT NULL,
    group_type text NOT NULL,
    state text NOT NULL CHECK (state IN ('running', 'done')),
    -- The last group id done; NULL before the first page.
    after_id text,
    index_version smallint NOT NULL,
    started_at timestamptz NOT NULL DEFAULT now(),
    finished_at timestamptz,
    PRIMARY KEY (workspace_id, group_type)
);

DO $$
DECLARE
    t text;
BEGIN
    FOREACH t IN ARRAY ARRAY['groups', 'entries', 'backfills']
    LOOP
        EXECUTE format('ALTER TABLE search.%I ENABLE ROW LEVEL SECURITY', t);
        EXECUTE format('ALTER TABLE search.%I FORCE ROW LEVEL SECURITY', t);
        EXECUTE format(
            'CREATE POLICY workspace_isolation ON search.%I USING '
            '(workspace_id = nullif(current_setting(''campbooks.workspace_id'', true), '''')::uuid)',
            t);
    END LOOP;
END
$$;

-- The runtime role (see `kernel__runtime_role`).
GRANT USAGE ON SCHEMA search TO campbooks_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA search TO campbooks_app;
GRANT EXECUTE ON FUNCTION search.entry_vector(text, text, text, text, text) TO campbooks_app;
