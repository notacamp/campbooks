-- What the schedule module's use cases need beyond the first schema
-- (campbooks-cloud#42, tasks 6.x): the member's answer to an invitation, the
-- notice a failed or superseded change leaves, a delete the member can still
-- undo, an event hidden because it was pushed to a provider calendar, the
-- last provider version of an event (restored when a change is not kept), and
-- the requests connectors is asked to carry out, bound to the request that
-- made them (campbooks-cloud#227).
SET LOCAL lock_timeout = '10s';

ALTER TABLE schedule.events
    ADD COLUMN response text
        CHECK (response IN ('needs_action', 'accepted', 'tentative', 'declined')),
    -- Why the member's last change is not what the event shows now.
    ADD COLUMN notice text CHECK (notice IN ('kept_theirs', 'not_saved')),
    -- A delete the member can still undo: the event is hidden, and the
    -- delete reaches the provider only once the undo window has passed.
    ADD COLUMN deleted_at timestamptz,
    ADD COLUMN undo_token uuid,
    -- The provider copy an "Add to my calendar" made; this Campbooks event
    -- is hidden while it stands.
    ADD COLUMN pushed_to uuid,
    -- Whether the event counts when free time is worked out.
    ADD COLUMN busy boolean NOT NULL DEFAULT true,
    -- The event as the provider last had it (title, place, notes, times,
    -- status, recurrence), for a change the provider did not keep.
    ADD COLUMN provider_version jsonb,
    -- Every key of a read item that made or changed this event (`item_key` is
    -- the first): a later email's `updates` and `cancels` name the item they
    -- change, whichever of its keys, and a delivery repeated is recognised.
    ADD COLUMN item_keys text[] NOT NULL DEFAULT '{}',
    -- The conversation of the source message (an id of the conversations
    -- module), so the event can be explained with its email.
    ADD COLUMN source_conversation uuid,
    -- The sentence of the email that gave the date (a model read only).
    ADD COLUMN quote text;

CREATE INDEX events_item_keys_idx ON schedule.events USING gin (item_keys);
CREATE INDEX events_deleted_idx ON schedule.events (workspace_id, deleted_at)
    WHERE deleted_at IS NOT NULL;

ALTER TABLE schedule.member_settings
    -- The provider calendar the member last pushed to, offered first.
    ADD COLUMN last_push_calendar uuid;

-- A change to an event on a provider calendar that connectors is asked to
-- make. The port the kernel gives connectors answers from this row, after
-- verifying its binding.
CREATE TABLE schedule.change_requests (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL,
    -- The member who asked.
    member_id uuid NOT NULL,
    calendar_id uuid NOT NULL,
    event_id uuid NOT NULL,
    connection_id uuid NOT NULL,
    change text NOT NULL
        CHECK (change IN ('create', 'update', 'update_occurrence', 'delete', 'respond')),
    -- The `calendar-change` v1 blob reference.
    body jsonb NOT NULL,
    mac bytea,
    state text NOT NULL DEFAULT 'waiting'
        CHECK (state IN ('waiting', 'applied', 'superseded', 'skipped', 'failed')),
    created_at timestamptz NOT NULL DEFAULT now(),
    finished_at timestamptz
);
CREATE INDEX change_requests_event_idx ON schedule.change_requests (workspace_id, event_id);

ALTER TABLE schedule.change_requests ENABLE ROW LEVEL SECURITY;
ALTER TABLE schedule.change_requests FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON schedule.change_requests USING
    (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);
GRANT SELECT, INSERT, UPDATE, DELETE ON schedule.change_requests TO campbooks_app;
