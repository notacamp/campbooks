-- Task items (campbooks-cloud#43): a task planned Later stays off Home until
-- this time, and comes back by itself. Catalog-only: a nullable column.
SET LOCAL lock_timeout = '10s';

ALTER TABLE attention.items ADD COLUMN hidden_until timestamptz;
