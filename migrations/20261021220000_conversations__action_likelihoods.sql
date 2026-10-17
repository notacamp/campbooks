-- How likely the member is to want each action of the catalogue for a
-- message (campbooks-cloud#166): written by the model pass and read when a
-- need is shown, both only while `CAMPBOOKS_CONVERSATIONS__ACTIONS__ENABLED`
-- is on (the floor applies when read). A likelihood and the model that gave
-- it, nothing of the message itself. Every row carries its
-- workspace (forced row-level security) and goes with its message. Only new
-- objects are created; the timeout keeps an unexpected lock wait from
-- holding up a deploy.
SET LOCAL lock_timeout = '5s';

CREATE TABLE conversations.action_likelihoods (
    message_id uuid NOT NULL REFERENCES conversations.messages (id) ON DELETE CASCADE,
    workspace_id uuid NOT NULL,
    -- `reply`, `remind`, `open_attachment`, `forward`, `report`, `safe`,
    -- `already_paid`, `still_to_pay` (campbooks_ai::decide::signals::actions).
    action text NOT NULL CHECK (action IN ('reply', 'remind', 'open_attachment', 'forward',
                                           'report', 'safe', 'already_paid', 'still_to_pay')),
    likelihood double precision NOT NULL CHECK (likelihood >= 0 AND likelihood <= 1),
    -- The model that answered, as the provider names it.
    model text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (message_id, action)
);

ALTER TABLE conversations.action_likelihoods ENABLE ROW LEVEL SECURITY;
ALTER TABLE conversations.action_likelihoods FORCE ROW LEVEL SECURITY;
CREATE POLICY workspace_isolation ON conversations.action_likelihoods USING
    (workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid);

GRANT SELECT, INSERT, UPDATE, DELETE ON conversations.action_likelihoods TO campbooks_app;
