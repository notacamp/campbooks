-- When each change of the client feed was numbered (campbooks-cloud#131).
-- Changes are pruned by this time instead of when they were published, so
-- a resume token's age alone says whether every change after it is still
-- kept: whether a token has expired then depends on nothing the caller may
-- not see (the time a change in a hidden inbox was published, how many
-- there were).
--
-- The ALTER takes an ACCESS EXCLUSIVE lock on kernel.changes and holds it
-- through the backfill and the index build below, until this migration
-- commits: producers and streams of every workspace wait meanwhile. The
-- table holds the retention's worth of changes (72 hours by default), so
-- this takes a moment, not minutes.
SET LOCAL lock_timeout = '5s';

ALTER TABLE kernel.changes ADD COLUMN sequenced_at timestamptz;

-- Changes numbered before now keep the age they were pruned by: every
-- workspace's (the kernel scope; a migration login that owns the table is
-- subject to its forced row-level security too).
SET LOCAL campbooks.kernel_scope = 'on';
UPDATE kernel.changes SET sequenced_at = occurred_at WHERE seq IS NOT NULL;

ALTER TABLE kernel.changes
    ADD CONSTRAINT changes_sequenced_at_check CHECK ((seq IS NULL) = (sequenced_at IS NULL));

-- Pruning by age; unnumbered changes are found through
-- `changes_unsequenced_idx`.
CREATE INDEX changes_sequenced_at_idx ON kernel.changes (sequenced_at) WHERE seq IS NOT NULL;
DROP INDEX kernel.changes_occurred_at_idx;
