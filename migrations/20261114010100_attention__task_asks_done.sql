-- "You replied. Done?" (campbooks-cloud#396): a task that asks the member
-- whether it is done stays on Home whatever its plan, so the question is
-- seen (`tasks.task_changed.asks_done`). Catalog-only: a column with a
-- constant default.
SET LOCAL lock_timeout = '10s';

ALTER TABLE attention.items ADD COLUMN asks_done boolean NOT NULL DEFAULT false;
