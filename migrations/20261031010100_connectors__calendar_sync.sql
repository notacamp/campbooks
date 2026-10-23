-- Calendar sync (campbooks-cloud#42, `add-calendar-sync`).
--
-- A connection with the `calendar` capability has one calendar stream (the
-- `streams` row of capability `calendar`). `calendars` are the provider
-- calendars that stream knows: whether each is read (`syncing`, turned on
-- and off from the schedule module's `calendar_sync_changed`) and where the
-- provider's sync token for it stands. `calendar_changes` are the changes
-- members asked for (`schedule.outbound_requested`), recorded once the
-- schedule module confirmed them, bound to the request with a MAC under a key
-- the database never holds (campbooks-cloud#227), and made by the stream's
-- next run before it reads. The provider's own calendar and event ids are
-- text here and nowhere in an event: this schema is the module's own.
--
-- Wait at most 5 s for a lock: a busy instance fails this migration fast,
-- and the next start retries it, rather than stalling.
SET LOCAL lock_timeout = '5s';

-- Why a stream does not work for its member to act on, when its connection
-- is otherwise fine (the calendar API is not enabled for the app): the
-- connection's health says nothing of one capability.
ALTER TABLE connectors.streams ADD COLUMN IF NOT EXISTS health_reason text;

CREATE TABLE connectors.calendars (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    stream_id uuid NOT NULL REFERENCES connectors.streams (id) ON DELETE CASCADE,
    -- The calendar's id at the provider (it can be an address).
    provider_calendar_id text NOT NULL,
    -- The schedule module's id of it, once that module said (an id only).
    schedule_calendar uuid,
    -- Whether it is read. The primary calendar at first; the others when the
    -- member turns them on.
    syncing boolean NOT NULL DEFAULT false,
    -- The provider's sync token; none until the first full read finished.
    sync_token text,
    last_full_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (stream_id, provider_calendar_id)
);
CREATE INDEX calendars_schedule_idx ON connectors.calendars (schedule_calendar)
    WHERE schedule_calendar IS NOT NULL;

CREATE TABLE connectors.calendar_changes (
    -- UUIDv7: the order changes were recorded in, which is the order they
    -- are made in.
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    stream_id uuid NOT NULL REFERENCES connectors.streams (id) ON DELETE CASCADE,
    -- The schedule module's request (the id of its `change_requests` row).
    request_id uuid NOT NULL UNIQUE,
    calendar_id uuid NOT NULL,
    event_id uuid NOT NULL,
    change text NOT NULL
        CHECK (change IN ('create', 'update', 'update_occurrence', 'delete', 'respond')),
    -- The `calendar-change` v1 blob reference the schedule module stored and
    -- verified: what is written is that document and no other.
    body jsonb NOT NULL,
    mac bytea,
    state text NOT NULL DEFAULT 'pending'
        CHECK (state IN ('pending', 'applied', 'superseded', 'skipped', 'failed')),
    -- Why it ended as it did (a stable code, never provider words).
    reason text,
    -- Runs that met provider trouble with it.
    attempts integer NOT NULL DEFAULT 0,
    -- What the provider answered when it was applied; the next read does not
    -- report that version back (no echo).
    provider_event_id text,
    etag text,
    created_at timestamptz NOT NULL DEFAULT now(),
    done_at timestamptz
);
CREATE INDEX calendar_changes_pending_idx ON connectors.calendar_changes (stream_id, id)
    WHERE state = 'pending';
CREATE INDEX calendar_changes_done_idx ON connectors.calendar_changes (done_at)
    WHERE state <> 'pending';

ALTER TABLE connectors.calendars ENABLE ROW LEVEL SECURITY;
ALTER TABLE connectors.calendars FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON connectors.calendars USING
    (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);
GRANT SELECT, INSERT, UPDATE, DELETE ON connectors.calendars TO campbooks_app;

ALTER TABLE connectors.calendar_changes ENABLE ROW LEVEL SECURITY;
ALTER TABLE connectors.calendar_changes FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON connectors.calendar_changes USING
    (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);
GRANT SELECT, INSERT, UPDATE, DELETE ON connectors.calendar_changes TO campbooks_app;
