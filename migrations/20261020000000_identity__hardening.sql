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

GRANT SELECT, INSERT, UPDATE, DELETE ON identity.notices TO campbooks_app;

-- An authorization request belongs to the browser that made it: the SHA-256
-- of that browser's `__Host-campbooks_browser` cookie. Requests waiting at
-- upgrade time (at most 15 minutes old) have none and are dropped: those
-- sign-ins start again from the app.
DELETE FROM identity.authorize_requests;
ALTER TABLE identity.authorize_requests ADD COLUMN browser_hash bytea NOT NULL;
