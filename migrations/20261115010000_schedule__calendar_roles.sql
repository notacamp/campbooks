-- Calendar roles (campbooks-cloud#399): a provider calendar is the member's
-- own (mine) or one they only follow. The role is derived: off = not
-- syncing, following = syncing and `following`, mine = syncing and not
-- `following`. `owned` is the provider's word on whether the account owns
-- the calendar, kept apart from the member's choice; NULL means unknown.
SET LOCAL lock_timeout = '10s';

ALTER TABLE schedule.calendars
    ADD COLUMN IF NOT EXISTS following boolean NOT NULL DEFAULT false,
    ADD COLUMN IF NOT EXISTS owned boolean;

-- The Campbooks calendar is always mine.
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint
                    WHERE conname = 'calendars_following_provider_only'
                      AND conrelid = 'schedule.calendars'::regclass) THEN
        ALTER TABLE schedule.calendars
            ADD CONSTRAINT calendars_following_provider_only
            CHECK (source = 'provider' OR NOT following);
    END IF;
END
$$;

-- A calendar that cannot be written is never the member's own: a holidays
-- calendar, or a colleague's shared read-only. Every other calendar stays as
-- it was, so nothing that counts today stops counting unasked.
UPDATE schedule.calendars SET following = true WHERE source = 'provider' AND NOT writable;
