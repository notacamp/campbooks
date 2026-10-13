-- What a conversation needs from its inbox's members (attention, campbooks-cloud#39):
-- at most one need per conversation, derived from the newest inbound message's
-- decisions and the conversation's state, and the member's verdict on it.

CREATE TABLE conversations.needs (
    conversation_id uuid PRIMARY KEY REFERENCES conversations.conversations (id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL,
    -- The inbound message the need comes from (no foreign key: when the
    -- provider removes it, the need is cleared and announced, not dropped).
    message_id uuid NOT NULL,
    reason text NOT NULL CHECK (reason IN ('reply', 'do', 'pay', 'review')),
    trust text NOT NULL CHECK (trust IN ('sure', 'check')),
    basis text NOT NULL CHECK (basis IN ('rules', 'model', 'member')),
    -- The question that raised it, and the answer whose reasons explain it.
    question text NOT NULL CHECK (question IN ('needs_reply', 'asks_action', 'suspicious')),
    decision_id uuid REFERENCES conversations.decisions (id) ON DELETE SET NULL,
    due_on date,
    urgent boolean NOT NULL DEFAULT false,
    state text NOT NULL CHECK (state IN ('open', 'cleared')),
    cleared_because text CHECK (cleared_because IN
        ('replied', 'archived', 'trashed', 'done', 'not_needed', 'superseded')),
    -- A user id (workspaces module) when a member cleared it.
    cleared_by uuid,
    cleared_at timestamptz,
    -- A member's verdict on the need raised by `verdict_message_id`: it is
    -- done, not needed, or confirmed (a question answered yes).
    verdict text CHECK (verdict IN ('done', 'not_needed', 'confirmed')),
    verdict_message_id uuid,
    verdict_by uuid,
    verdict_correction_id uuid,
    -- Grows with every change; consumers apply only newer revisions.
    revision bigint NOT NULL DEFAULT 1,
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK ((state = 'open') = (cleared_because IS NULL))
);
CREATE INDEX needs_open_idx ON conversations.needs (workspace_id) WHERE state = 'open';

-- A correction a member took back (Not needed, then Undo).
ALTER TABLE conversations.corrections ADD COLUMN withdrawn_at timestamptz;

ALTER TABLE conversations.needs ENABLE ROW LEVEL SECURITY;
ALTER TABLE conversations.needs FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON conversations.needs USING
    (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);

GRANT SELECT, INSERT, UPDATE, DELETE ON conversations.needs TO campbooks_app;
