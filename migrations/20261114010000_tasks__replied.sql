-- "You replied. Done?" (campbooks-cloud#396): when the member replies in the
-- conversation an open Do task came from, the task remembers the reply (when
-- it was sent and which message it was, a conversations record referenced by
-- id only) so Home, Tasks and the thread can ask whether it is done. Not yet
-- is recorded as `reply_answered_at`: the question is asked again only after
-- a newer reply. Catalog-only: nullable columns and a wider check.
SET LOCAL lock_timeout = '10s';

ALTER TABLE tasks.tasks
    ADD COLUMN replied_at timestamptz,
    ADD COLUMN replied_message uuid,
    ADD COLUMN reply_answered_at timestamptz;

-- The history says when a reply was noticed and how the question was answered.
ALTER TABLE tasks.history DROP CONSTRAINT history_what_check;
ALTER TABLE tasks.history ADD CONSTRAINT history_what_check
    CHECK (what IN ('created', 'edited', 'due_changed', 'planned', 'done', 'not_needed',
                    'reopened', 'handed_off', 'taken_back', 'held', 'released',
                    'replied', 'reply_answered', 'reply_unanswered'));
