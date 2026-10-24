-- Whose calendar a provider calendar is (`google` or `zoho`), from the
-- calendar list of the grant, so the event sheet can name it. Calendars made
-- before this have none.
SET LOCAL lock_timeout = '10s';

ALTER TABLE schedule.calendars
    ADD COLUMN provider text CHECK (provider IN ('google', 'zoho'));
