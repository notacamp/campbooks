-- Idempotency keys for mutating operations (ADR 0013). One row per
-- (principal, key): the operation and a hash of the request it was first used
-- with, and once that request finished, the response to replay. Rows expire
-- after a day and are deleted by the `kernel.prune_idempotency` schedule.
CREATE TABLE kernel.idempotency (
    -- `user:<subject>` or `client:<client id>`: keys are per caller.
    principal text NOT NULL,
    -- The `Idempotency-Key` header value.
    key text NOT NULL,
    -- The operation (`operationId`) the key was first used with.
    operation_id text NOT NULL,
    -- SHA-256 of the first request's method, path, query and body.
    request_hash bytea NOT NULL,
    -- `pending` while the first request runs, `completed` once its response
    -- is stored.
    state text NOT NULL DEFAULT 'pending' CHECK (state IN ('pending', 'completed')),
    response_status smallint,
    response_content_type text,
    response_location text,
    response_body bytea,
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    PRIMARY KEY (principal, key),
    CHECK (state = 'pending' OR (response_status IS NOT NULL AND response_body IS NOT NULL))
);

-- Pruning scans by expiry.
CREATE INDEX idempotency_expires_at_idx ON kernel.idempotency (expires_at);
