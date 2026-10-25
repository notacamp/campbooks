-- Home ordered by priority bands (campbooks-cloud#358). `attention.list`
-- puts each open item in Now, Today, This week or Whenever from facts the
-- producing modules publish, kept here with the item:
--
-- - `warning`: the need is the warning about suspicious mail
--   (`conversations.need_changed`);
-- - `from_person`: a person sent the need's message rather than a machine,
--   from the headers alone; NULL when unknown (an event written before);
-- - `plan` and `plan_on`: a task's plan (`none`, `date`, `later`) and its
--   planned day (`tasks.task_changed`); NULL for a task projected before,
--   which Home keeps showing as it did until the task next changes.
--
-- Catalog-only: nullable columns and a constant default.

-- Wait at most 10 s for a lock: a busy instance fails this migration fast,
-- and the next start retries it, rather than stalling.
SET LOCAL lock_timeout = '10s';

ALTER TABLE attention.items
    ADD COLUMN warning boolean NOT NULL DEFAULT false,
    ADD COLUMN from_person boolean,
    ADD COLUMN plan text,
    ADD COLUMN plan_on date;
