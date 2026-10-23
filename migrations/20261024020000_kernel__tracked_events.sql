SET LOCAL lock_timeout = '5s';

-- The tracked events outbox (ADR 0020, campbooks-cloud#320).
--
-- `Tracker::track` appends one row per tracked event in the transaction of
-- the change, so a change that rolls back leaves no event and a committed
-- change always has its event. A relay (campbooks-cloud#321) publishes the
-- rows to the broker and sets `published_at`; until then, with
-- `CAMPBOOKS_TRACKING__SINK=outbox`, rows only accumulate, and with the
-- default `none` nothing is written at all.
--
-- A row holds ids, enums, counts and times only: the envelope (actor and on
-- behalf of as account or client ids, operation, correlation, causation,
-- role, versions) and the body (properties and record snapshots), both
-- checked against the tracking plan, which has no free-text type and
-- rejects personal data and content.
CREATE TABLE kernel.tracked_events (
    -- The event's id (UUIDv7, chosen by the kernel): subscribers deduplicate
    -- on it.
    id uuid PRIMARY KEY,
    -- NULL for an instance-wide event.
    workspace_id uuid,
    name text NOT NULL CHECK (name ~ '^[a-z][a-z0-9_]*\.[a-z][a-z0-9_]*$'),
    version integer NOT NULL CHECK (version >= 1),
    occurred_at timestamptz NOT NULL,
    envelope jsonb NOT NULL CHECK (jsonb_typeof(envelope) = 'object'),
    body jsonb NOT NULL CHECK (jsonb_typeof(body) = 'object'),
    -- Set by the relay once the broker confirmed it (campbooks-cloud#321).
    published_at timestamptz
);

-- What the relay reads next, in order.
CREATE INDEX tracked_events_unpublished_idx ON kernel.tracked_events (id)
    WHERE published_at IS NULL;

-- Row-level security like the event log's: a workspace transaction appends
-- only its own workspace's rows; the kernel scope (the kernel lifting an
-- instance-wide event's insert, and the relay to come) reaches every row;
-- a transaction that set neither reaches none. The runtime role may only
-- append: it cannot read, change or delete a row back, so nothing in the
-- application rewrites the record (the relay reads through a definer
-- function, campbooks-cloud#321).
ALTER TABLE kernel.tracked_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE kernel.tracked_events FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON kernel.tracked_events
    USING (current_setting('campbooks.kernel_scope', true) = 'on'
           OR workspace_id::text = nullif(current_setting('campbooks.workspace_id', true), ''))
    WITH CHECK (current_setting('campbooks.kernel_scope', true) = 'on'
                OR workspace_id::text = nullif(current_setting('campbooks.workspace_id', true), ''));

GRANT INSERT ON kernel.tracked_events TO campbooks_app;
