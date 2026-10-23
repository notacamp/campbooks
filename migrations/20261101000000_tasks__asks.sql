-- Tasks Scout made from the asks of an email (campbooks-cloud#43): the key of
-- the ask a task came from, so a repeated delivery of the same read makes
-- nothing twice.
ALTER TABLE tasks.tasks ADD COLUMN ask_key text;
CREATE UNIQUE INDEX tasks_ask_key_idx ON tasks.tasks (workspace_id, ask_key)
    WHERE ask_key IS NOT NULL;
