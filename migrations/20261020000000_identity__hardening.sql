-- Identity hardening (security audit, batch 3).
--
-- `notices`: which security notices were sent, so a retried delivery never
-- sends one twice. One row per event and recipient (`account`: the
-- account's primary address; `way_in`: the new way in's address). The
-- address itself is never stored here.
CREATE TABLE identity.notices (
    event_id uuid NOT NULL,
    recipient text NOT NULL CHECK (recipient IN ('account', 'way_in')),
    account_id uuid NOT NULL REFERENCES identity.accounts (id) ON DELETE CASCADE,
    sent_at timestamptz NOT NULL,
    PRIMARY KEY (event_id, recipient)
);
CREATE INDEX notices_account_idx ON identity.notices (account_id);

GRANT SELECT, INSERT, DELETE ON identity.notices TO campbooks_app;
