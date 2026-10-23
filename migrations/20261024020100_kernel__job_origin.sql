-- Who caused a job (ADR 0020, campbooks-cloud#320): the ambient tracking
-- context when the job was enqueued or the delivery fanned out, so the
-- events the job tracks name the person (or app) it works for and carry
-- that request's correlation id.
--
-- `{"actor": {"type": "user", "id": "<account id>"}, "correlation": "<trace id>"}`:
-- ids and enums only, at most 1 KB (the kernel drops a larger one rather
-- than refuse the enqueue). Its fields are audited like job payloads'
-- (`xtask/src/payloads.rs`, `JOB_ORIGIN_FIELDS`). NULL for work no request
-- caused (a schedule's tick, a replay).
--
-- Nullable, no default: adding it rewrites nothing. The check is added NOT
-- VALID, so the queue's lock is held only briefly; the next migration
-- validates it in a transaction of its own. The timeout keeps an unexpected
-- lock wait from holding up a deploy.
SET LOCAL lock_timeout = '5s';

ALTER TABLE kernel.jobs ADD COLUMN IF NOT EXISTS origin jsonb;
ALTER TABLE kernel.jobs ADD CONSTRAINT jobs_origin_check CHECK (
    origin IS NULL
    OR (jsonb_typeof(origin) = 'object' AND octet_length(origin::text) <= 1024)
) NOT VALID;
