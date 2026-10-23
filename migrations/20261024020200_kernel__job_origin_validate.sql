-- Validate the check `…120100_kernel__job_origin` added NOT VALID
-- (campbooks-cloud#320). A migration of its own, so it runs in a transaction
-- of its own: VALIDATE CONSTRAINT takes only SHARE UPDATE EXCLUSIVE, so the
-- queue keeps working while it scans the rows (all NULL).

-- Never wait long for a lock on a busy table: fail and retry the deploy.
SET LOCAL lock_timeout = '5s';

ALTER TABLE kernel.jobs VALIDATE CONSTRAINT jobs_origin_check;
