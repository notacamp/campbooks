-- When what an item asks stops being possible (campbooks-cloud#275): carried
-- by `conversations.need_changed`. `attention.list` leaves an item out once
-- this time has passed, without waiting for the producer to clear it.
-- Catalog-only: a nullable column.
SET LOCAL lock_timeout = '10s';

ALTER TABLE attention.items ADD COLUMN lapses_at timestamptz;
