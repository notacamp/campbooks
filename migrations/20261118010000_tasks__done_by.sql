-- How a task gets done (campbooks-cloud#396): Scout's read says whether
-- replying to the email is itself how an ask is done ('reply': an answer, an
-- approval, a choice) or whether it is done some other way ('elsewhere': sign
-- and send a form, file, upload, go). "Done?" is asked after a reply only for
-- the first kind. NULL for tasks made before it was read, which keep asking.
-- An enum fact, no text: the sender's own sentence is never kept here, it is
-- served by the conversation under the conversation's own access check.
-- Catalog-only: a nullable column and a check.
SET LOCAL lock_timeout = '10s';

ALTER TABLE tasks.tasks
    ADD COLUMN done_by text CHECK (done_by IN ('reply', 'elsewhere'));
