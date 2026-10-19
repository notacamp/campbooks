-- "Worth a look" (campbooks-cloud#274): a new reason, shown after Pay.

-- Wait at most 10 s for a lock: a busy instance fails this migration fast,
-- and the next start retries it, rather than stalling.
SET LOCAL lock_timeout = '10s';

-- Widened NOT VALID, validated by `…000300_attention__worth_a_look_validate`.
ALTER TABLE attention.items DROP CONSTRAINT items_reason_check;
ALTER TABLE attention.items ADD CONSTRAINT items_reason_check
    CHECK (reason IN ('review', 'reply', 'decide', 'do', 'pay', 'worth_a_look')) NOT VALID;
