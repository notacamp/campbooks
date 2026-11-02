-- Calendar lists are read again (campbooks-cloud#405): a calendar stream
-- reads its account's calendar list about once a day, so a calendar shared
-- later reaches the schedule module. `calendar_list_due_at` is when the next
-- read is due (NULL: now).
SET LOCAL lock_timeout = '10s';

ALTER TABLE connectors.streams
    ADD COLUMN IF NOT EXISTS calendar_list_due_at timestamptz;

-- Spread the streams that exist over the next day, so a deploy does not read
-- every list at once.
UPDATE connectors.streams
   SET calendar_list_due_at = now() + random() * interval '24 hours'
 WHERE capability = 'calendar' AND calendar_list_due_at IS NULL;
