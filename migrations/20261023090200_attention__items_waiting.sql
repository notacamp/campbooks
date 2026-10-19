-- Who has the next move (campbooks-cloud#293): two new reasons. `waiting`
-- items stay out of the queue (`attention.list` `state=waiting` lists them);
-- `nudge` items are listed after Pay and before Worth a look.

-- Wait at most 10 s for a lock: a busy instance fails this migration fast,
-- and the next start retries it, rather than stalling.
SET LOCAL lock_timeout = '10s';

-- Widened NOT VALID, then validated in the same transaction: the table is
-- one row per conversation with a need, so the scan is short, and the one
-- slot this change has for the module keeps it to one migration.
ALTER TABLE attention.items DROP CONSTRAINT items_reason_check;
ALTER TABLE attention.items ADD CONSTRAINT items_reason_check
    CHECK (reason IN ('review', 'reply', 'decide', 'do', 'pay', 'worth_a_look', 'waiting',
                      'nudge')) NOT VALID;
ALTER TABLE attention.items VALIDATE CONSTRAINT items_reason_check;
