-- Time held for a task on the member's calendar (campbooks-cloud#362): the
-- schedule module's event and the calendar it is on (referenced by id only,
-- no cross-schema key) and its times, set together or not at all. One hold
-- per task, and one task per event.
ALTER TABLE tasks.tasks
    ADD COLUMN hold_event uuid,
    ADD COLUMN hold_calendar uuid,
    ADD COLUMN held_from timestamptz,
    ADD COLUMN held_to timestamptz,
    ADD CONSTRAINT tasks_hold_all_or_none CHECK (
        (hold_event IS NULL AND hold_calendar IS NULL AND held_from IS NULL AND held_to IS NULL)
        OR (hold_event IS NOT NULL AND hold_calendar IS NOT NULL AND held_from IS NOT NULL
            AND held_to IS NOT NULL AND held_to > held_from)
    );
CREATE UNIQUE INDEX tasks_hold_event_idx ON tasks.tasks (workspace_id, hold_event)
    WHERE hold_event IS NOT NULL;

-- The history says when time was held and released.
ALTER TABLE tasks.history DROP CONSTRAINT history_what_check;
ALTER TABLE tasks.history ADD CONSTRAINT history_what_check
    CHECK (what IN ('created', 'edited', 'due_changed', 'planned', 'done', 'not_needed',
                    'reopened', 'handed_off', 'taken_back', 'held', 'released'));
