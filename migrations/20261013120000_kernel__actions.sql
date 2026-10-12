-- Kernel infrastructure for the action registry (ADR 0008): idempotency keys
-- and undo tokens of the generated `/api/v1/actions/*` endpoints. Written only
-- by `campbooks_kernel::declare`; module migrations may not touch them.

-- One row per `Idempotency-Key` a caller sent to a mutating action. Inserted
-- in the action's transaction before its handler runs and completed with the
-- response in that same transaction, so a committed row always carries its
-- response, and a request that failed leaves no row (the key can be retried).
-- A concurrent request with the same key waits on the uncommitted row, then
-- replays the stored response.
CREATE TABLE kernel.idempotency_keys (
    -- The caller: `user:<sub>` or `client:<client id>`.
    principal text NOT NULL,
    -- The `Idempotency-Key` header value.
    key text NOT NULL,
    -- The action it was used with (`<module>.<verb>`).
    action text NOT NULL,
    -- The caller's workspace when the request was made, if any.
    workspace_id text,
    -- SHA-256 of the decoded input, re-serialized: a reused key with another
    -- input is refused rather than replayed.
    request_hash bytea NOT NULL,
    -- The response body sent (and replayed). NULL only inside the request's
    -- own uncommitted transaction.
    response jsonb,
    created_at timestamptz NOT NULL DEFAULT now(),
    -- After this the key is forgotten (pruned hourly by `kernel.prune_actions`).
    expires_at timestamptz NOT NULL,
    PRIMARY KEY (principal, key)
);

CREATE INDEX idempotency_keys_expires_at_idx ON kernel.idempotency_keys (expires_at);

-- One row per undo token handed out by an undoable action. The token itself
-- is never stored, only its SHA-256.
CREATE TABLE kernel.undo_tokens (
    token_hash bytea PRIMARY KEY,
    -- The action to undo (`<module>.<verb>`).
    action text NOT NULL,
    -- Who may redeem it: the caller of the action (`user:<sub>`/`client:<id>`)…
    principal text NOT NULL,
    -- …in the same workspace.
    workspace_id text,
    -- What the action's undo handler needs (ids and prior state, never secrets).
    payload jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    -- Set when redeemed: a token undoes once.
    redeemed_at timestamptz
);

CREATE INDEX undo_tokens_expires_at_idx ON kernel.undo_tokens (expires_at);
