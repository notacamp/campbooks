-- Mail and Scout in pages (pages-from-mail-and-scout, campbooks-cloud#364).
--
-- A file a page holds because a member saved a message's attachment to it
-- keeps where it came from (`source`: the attachment's record type and id),
-- so taking that back (`pages.detach_files`) removes only what mail put
-- there, never a file the member uploaded. The bytes are not copied: the
-- page holds the same blob as the message.
--
-- `pages.appends` remembers what an append (`pages.append`) added to a page:
-- each block's stable id and a hash of what it held, so taking the append
-- back removes the blocks nobody has changed since. Hashes and ids only,
-- never any of the text.
--
-- Written to run again safely: every object is created only when missing.
--
-- Wait at most 5 s for a lock: a busy instance fails this migration fast,
-- and the next start retries it, rather than stalling.
SET LOCAL lock_timeout = '5s';

ALTER TABLE pages.files
    ADD COLUMN IF NOT EXISTS source_type text,
    ADD COLUMN IF NOT EXISTS source_id text;
ALTER TABLE pages.files DROP CONSTRAINT IF EXISTS files_source_whole;
ALTER TABLE pages.files ADD CONSTRAINT files_source_whole CHECK (
    (source_type IS NULL AND source_id IS NULL)
    OR (source_type IS NOT NULL AND source_id IS NOT NULL
        AND char_length(source_type) <= 64 AND char_length(source_id) <= 64)
);

CREATE TABLE IF NOT EXISTS pages.appends (
    id uuid PRIMARY KEY,
    page_id uuid NOT NULL REFERENCES pages.pages (id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL,
    appended_by uuid NOT NULL,
    at timestamptz NOT NULL DEFAULT now(),
    -- [{"bid": uuid, "hash": hex}, ...] in the order they were added.
    units jsonb NOT NULL CHECK (jsonb_typeof(units) = 'array')
);
CREATE INDEX IF NOT EXISTS appends_page_idx ON pages.appends (page_id, at);

ALTER TABLE pages.appends ENABLE ROW LEVEL SECURITY;
ALTER TABLE pages.appends FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS workspace_isolation ON pages.appends;
CREATE POLICY workspace_isolation ON pages.appends USING
    (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);
GRANT SELECT, INSERT, UPDATE, DELETE ON pages.appends TO campbooks_app;
