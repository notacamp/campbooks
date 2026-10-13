-- The W3C `traceparent` of the span that enqueued a job (or published the
-- event a delivery carries), so the job's span can link back to it. Ids and
-- flags only (55 ASCII characters); never user data. NULL when there was no
-- trace.
ALTER TABLE kernel.jobs
    ADD COLUMN trace_context text
        CHECK (trace_context IS NULL OR length(trace_context) <= 128);
