-- Kernel infrastructure: the durable event log and the job queue (ADR 0012).
-- The `kernel` schema belongs to `campbooks-kernel`, not to a module; module
-- migrations may not touch it (`cargo xtask arch`).
CREATE SCHEMA IF NOT EXISTS kernel;

-- A time-ordered UUID (version 7, RFC 9562) from a random v4 by overwriting
-- its first 48 bits with the Unix time in milliseconds and its version nibble
-- with 7. Same layout as `uuid::Uuid::now_v7()` in Rust. Postgres 18 has a
-- built-in `uuidv7()`; this keeps Postgres 17 supported.
CREATE FUNCTION kernel.uuid_v7() RETURNS uuid
LANGUAGE sql VOLATILE PARALLEL SAFE
AS $$
    SELECT encode(
        set_bit(
            set_bit(
                overlay(
                    uuid_send(gen_random_uuid())
                    PLACING substring(int8send(floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint) FROM 3)
                    FROM 1 FOR 6
                ),
                52, 1
            ),
            53, 1
        ),
        'hex'
    )::uuid
$$;

-- The event log: every domain event ever published, append-only and retained.
-- Written in the publisher's transaction together with the jobs that deliver
-- it to each registered consumer, so there is no event without its data change
-- and no data change without its event. Replay re-enqueues from here.
CREATE TABLE kernel.events (
    -- UUIDv7: unique without coordination and ordered by creation time.
    id uuid PRIMARY KEY DEFAULT kernel.uuid_v7(),
    -- `DomainEvent::NAME`, `<module>.<event>`.
    name text NOT NULL,
    -- `DomainEvent::VERSION`, the payload schema version.
    version integer NOT NULL CHECK (version > 0),
    -- The workspace the event belongs to; NULL for instance-wide events.
    workspace_id text,
    payload jsonb NOT NULL,
    occurred_at timestamptz NOT NULL DEFAULT now()
);

-- Replay scans one event name from a point in time.
CREATE INDEX events_name_occurred_at_idx ON kernel.events (name, occurred_at);

-- The job queue. Workers claim rows with `FOR UPDATE SKIP LOCKED`.
CREATE TABLE kernel.jobs (
    id uuid PRIMARY KEY DEFAULT kernel.uuid_v7(),
    -- Enqueue order; breaks ties and orders jobs sharing an `ordering_key`.
    seq bigint GENERATED ALWAYS AS IDENTITY,
    queue text NOT NULL,
    -- `Job::NAME` or `EventHandler::NAME`.
    kind text NOT NULL,
    payload jsonb NOT NULL,
    -- The workspace the work belongs to; NULL for instance-wide work.
    workspace_id text,
    -- Fairness bucket: the workspace, with instance-wide work as one bucket.
    workspace_key text GENERATED ALWAYS AS (coalesce(workspace_id, '')) STORED,
    -- For event deliveries: the event in `kernel.events` being delivered.
    event_id uuid,
    -- At most one unfinished job per (kind, unique_key).
    unique_key text,
    -- Jobs sharing an ordering key run one at a time, in enqueue order.
    ordering_key text,
    -- Higher runs first within a workspace.
    priority smallint NOT NULL DEFAULT 0,
    state text NOT NULL DEFAULT 'pending'
        CHECK (state IN ('pending', 'running', 'succeeded', 'failed')),
    run_at timestamptz NOT NULL DEFAULT now(),
    attempts integer NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    max_attempts integer NOT NULL CHECK (max_attempts > 0),
    -- The worker holding the job while it runs, and until when (heartbeat).
    locked_by text,
    locked_until timestamptz,
    -- The last failure, truncated; never carries secrets.
    last_error text,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    started_at timestamptz,
    finished_at timestamptz,
    CHECK ((state = 'running') = (locked_by IS NOT NULL AND locked_until IS NOT NULL))
);

-- Claiming: ready work per queue, walked one workspace at a time.
CREATE INDEX jobs_ready_idx ON kernel.jobs (queue, workspace_key, priority DESC, run_at, seq)
    WHERE state = 'pending';
-- Fairness: running jobs per workspace, and when each workspace was last served.
CREATE INDEX jobs_running_idx ON kernel.jobs (queue, workspace_key)
    WHERE state = 'running';
CREATE INDEX jobs_last_started_idx ON kernel.jobs (queue, workspace_key, started_at DESC)
    WHERE started_at IS NOT NULL;
-- Reclaiming expired leases.
CREATE INDEX jobs_lease_idx ON kernel.jobs (locked_until)
    WHERE state = 'running';
-- Idempotent enqueue.
CREATE UNIQUE INDEX jobs_unique_key_idx ON kernel.jobs (kind, unique_key)
    WHERE unique_key IS NOT NULL AND state IN ('pending', 'running');
-- Serial execution per ordering key.
CREATE INDEX jobs_ordering_idx ON kernel.jobs (ordering_key, seq)
    WHERE ordering_key IS NOT NULL AND state IN ('pending', 'running');
-- Pruning succeeded jobs; inspecting dead ones.
CREATE INDEX jobs_finished_idx ON kernel.jobs (state, finished_at)
    WHERE state IN ('succeeded', 'failed');

-- Recurring schedules: the last tick each schedule enqueued, so several
-- workers never enqueue the same tick twice.
CREATE TABLE kernel.schedules (
    name text PRIMARY KEY,
    last_tick bigint NOT NULL,
    updated_at timestamptz NOT NULL DEFAULT now()
);
