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
-- sign-ins start again from the app. Nullable so a build from before this
-- change (a rollback) can still store requests; a request without a hash
-- never matches one (`= NULL` is never true), so it is never completed by
-- this build.
LOCK TABLE identity.authorize_requests IN ACCESS EXCLUSIVE MODE;
DELETE FROM identity.authorize_requests;
ALTER TABLE identity.authorize_requests ADD COLUMN browser_hash bytea;

-- How a passkey was added: with the account (a passkey-only sign-up) or
-- later. Only a later one is a new way in worth pointing out. Existing
-- sign-up passkeys are the ones their account's creation names.
ALTER TABLE identity.passkeys
    ADD COLUMN added_via text NOT NULL DEFAULT 'passkey'
        CHECK (added_via IN ('sign_up', 'passkey'));
UPDATE identity.passkeys p
   SET added_via = 'sign_up'
  FROM identity.audit_events a
 WHERE a.kind = 'account_created'
   AND a.account_id = p.account_id
   AND a.metadata ->> 'passkey' = p.id::text;
