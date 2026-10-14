-- Model answers given before the decide role had an answers check
-- (campbooks-cloud#128) are marked as such, so they can be told apart from
-- answers given while a model passed it (`model_version` `checked:…`). They
-- are kept, never deleted; the read side ignores model answers that a check
-- did not vouch for. Idempotent: only rows without a version change.
--
-- The table forces row-level security even for its owner, so the owner
-- login running this migration lifts that for the one statement; the
-- migration's transaction hides the gap.
ALTER TABLE conversations.decisions NO FORCE ROW LEVEL SECURITY;
UPDATE conversations.decisions
   SET model_version = CASE WHEN model LIKE 'convaiinnovations/laya%'
                            THEN 'laya-0.3.21-unchecked'
                            ELSE 'unchecked' END
 WHERE source = 'model' AND model_version IS NULL;
ALTER TABLE conversations.decisions FORCE ROW LEVEL SECURITY;
