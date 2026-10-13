-- Rate-limit counters (`kernel::rate_limit`): one row per (limit, key) with
-- the hits of the current fixed window. `bucket` is an HMAC of the limit's
-- name and the key (an address, an account, a client) under a key derived
-- from CAMPBOOKS_SECRET_KEY: no address is stored. Ended windows are
-- deleted by the `kernel.prune_rate_limits` schedule.
CREATE TABLE kernel.rate_limits (
    bucket bytea PRIMARY KEY,
    hits integer NOT NULL,
    resets_at timestamptz NOT NULL
);

CREATE INDEX rate_limits_resets_at_idx ON kernel.rate_limits (resets_at);

-- Counters belong to callers, not workspaces: no row-level security.
GRANT SELECT, INSERT, UPDATE, DELETE ON kernel.rate_limits TO campbooks_app;
