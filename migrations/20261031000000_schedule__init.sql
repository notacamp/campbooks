-- The `schedule` module owns the `schedule` schema and nothing else
-- (campbooks-cloud#42). Other modules' records are referenced by ID only: no
-- cross-schema joins or foreign keys (checked by `cargo xtask arch`).
--
-- Calendars and their events: the provider calendars a calendar grant brings
-- and each member's own Campbooks calendar, which holds what Scout read from
-- email and which no provider sees. Recurring series are stored once, with the
-- provider's own rules, and expanded when events are listed. Row-level
-- security (ADR 0011) is keyed on `workspace_id`, set by the kernel for every
-- request and job.
SET LOCAL lock_timeout = '10s';

CREATE SCHEMA IF NOT EXISTS schedule;

-- A calendar belongs to one member of one workspace; only they see it.
CREATE TABLE schedule.calendars (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    -- The member (a `workspaces` user id).
    member_id uuid NOT NULL,
    source text NOT NULL CHECK (source IN ('campbooks', 'provider')),
    -- A provider calendar: the connection (a `connectors` id) and the
    -- calendar's id at the provider. A Campbooks calendar has neither.
    connection_id uuid,
    provider_calendar_id text,
    name text NOT NULL,
    color text,
    time_zone text,
    -- Whether its events are applied and shown.
    syncing boolean NOT NULL DEFAULT false,
    writable boolean NOT NULL DEFAULT true,
    is_primary boolean NOT NULL DEFAULT false,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK ((source = 'provider') = (connection_id IS NOT NULL AND provider_calendar_id IS NOT NULL))
);
-- Each member has exactly one Campbooks calendar.
CREATE UNIQUE INDEX calendars_campbooks_idx
    ON schedule.calendars (workspace_id, member_id) WHERE source = 'campbooks';
-- A provider calendar is added once per connection.
CREATE UNIQUE INDEX calendars_provider_idx
    ON schedule.calendars (workspace_id, connection_id, provider_calendar_id)
    WHERE source = 'provider';
CREATE INDEX calendars_member_idx ON schedule.calendars (workspace_id, member_id);

-- An event, or one occurrence replacing an occurrence of a series (an
-- exception), or a series (its `recurrence` set).
CREATE TABLE schedule.events (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    calendar_id uuid NOT NULL REFERENCES schedule.calendars (id) ON DELETE CASCADE,
    kind text NOT NULL DEFAULT 'none' CHECK (kind IN
        ('charge', 'appointment', 'meeting', 'travel', 'stay', 'show', 'reservation',
         'delivery', 'expiry', 'hold', 'none')),
    title text NOT NULL DEFAULT '',
    -- Timed events: instants (UTC). All-day events: dates (the end is
    -- exclusive, as the providers write it). Exactly one pair is set.
    starts_at timestamptz,
    ends_at timestamptz,
    starts_on date,
    ends_on date,
    all_day boolean NOT NULL DEFAULT false,
    -- The zone the event was written in (an IANA name), shown beside the
    -- member's own when different.
    time_zone text,
    place text,
    notes text,
    status text NOT NULL DEFAULT 'confirmed'
        CHECK (status IN ('confirmed', 'tentative', 'cancelled')),
    -- The provider's id and etag, and the member's change not yet written
    -- there (`create`, `update`, `delete`, `respond`).
    provider_event_id text,
    etag text,
    outbound_pending text
        CHECK (outbound_pending IN ('create', 'update', 'update_occurrence', 'delete', 'respond')),
    -- The provider's own recurrence lines (RRULE, RDATE, EXDATE), as given.
    recurrence text[] NOT NULL DEFAULT '{}',
    -- An exception points at its series and the occurrence it replaces.
    series_id uuid REFERENCES schedule.events (id) ON DELETE CASCADE,
    original_start text,
    -- The record it was read from (`conversations.message:<id>`).
    source_type text,
    source_id text,
    origin text NOT NULL CHECK (origin IN ('provider', 'member', 'scout')),
    found_by text CHECK (found_by IN ('structured', 'model')),
    -- The Campbooks event an "Add to my calendar" copied.
    pushed_from uuid,
    -- What makes a read item idempotent, and the iCalendar UID it carried.
    item_key text,
    ical_uid text,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK (all_day = (starts_on IS NOT NULL)),
    CHECK ((starts_at IS NOT NULL) <> (starts_on IS NOT NULL)),
    CHECK ((source_type IS NULL) = (source_id IS NULL))
);
CREATE INDEX events_calendar_idx ON schedule.events (workspace_id, calendar_id);
CREATE INDEX events_window_idx ON schedule.events (calendar_id, starts_at);
CREATE INDEX events_all_day_idx ON schedule.events (calendar_id, starts_on) WHERE all_day;
CREATE UNIQUE INDEX events_provider_idx
    ON schedule.events (calendar_id, provider_event_id) WHERE provider_event_id IS NOT NULL;
CREATE UNIQUE INDEX events_item_key_idx
    ON schedule.events (workspace_id, item_key) WHERE item_key IS NOT NULL;
CREATE INDEX events_series_idx ON schedule.events (series_id) WHERE series_id IS NOT NULL;
CREATE INDEX events_ical_uid_idx
    ON schedule.events (workspace_id, ical_uid) WHERE ical_uid IS NOT NULL;

-- A member's settings: their time zone (an IANA name, from the device once),
-- working hours per weekday, the kinds Scout adds, and per kind the calendar
-- an event is also added to.
CREATE TABLE schedule.member_settings (
    workspace_id uuid NOT NULL,
    member_id uuid NOT NULL,
    time_zone text,
    working_hours jsonb NOT NULL DEFAULT '{}' CHECK (jsonb_typeof(working_hours) = 'object'),
    scout_kinds text[] NOT NULL DEFAULT ARRAY['charge', 'appointment', 'meeting', 'travel',
        'stay', 'show', 'reservation', 'delivery', 'expiry'],
    also_add_to jsonb NOT NULL DEFAULT '{}' CHECK (jsonb_typeof(also_add_to) = 'object'),
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (workspace_id, member_id)
);

-- A Scout event the member removed: a re-read never adds it again, and it
-- stays as a labelled example of an unwanted read.
CREATE TABLE schedule.removed_items (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    member_id uuid NOT NULL,
    message_id uuid NOT NULL,
    item_key text NOT NULL,
    kind text NOT NULL,
    removed_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (workspace_id, member_id, item_key)
);

ALTER TABLE schedule.calendars ENABLE ROW LEVEL SECURITY;
ALTER TABLE schedule.calendars FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON schedule.calendars USING
    (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);
ALTER TABLE schedule.events ENABLE ROW LEVEL SECURITY;
ALTER TABLE schedule.events FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON schedule.events USING
    (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);
ALTER TABLE schedule.member_settings ENABLE ROW LEVEL SECURITY;
ALTER TABLE schedule.member_settings FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON schedule.member_settings USING
    (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);
ALTER TABLE schedule.removed_items ENABLE ROW LEVEL SECURITY;
ALTER TABLE schedule.removed_items FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON schedule.removed_items USING
    (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);

GRANT USAGE ON SCHEMA schedule TO campbooks_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON schedule.calendars TO campbooks_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON schedule.events TO campbooks_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON schedule.member_settings TO campbooks_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON schedule.removed_items TO campbooks_app;
