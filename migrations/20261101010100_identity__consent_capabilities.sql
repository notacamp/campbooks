-- A connect or reconnect may ask for more than the mailbox: the scopes of the
-- capabilities named here (`calendar`) join the consent (campbooks-cloud#42).
-- Every earlier ticket and flow asked for the mailbox alone, as the default
-- says.
SET LOCAL lock_timeout = '10s';

ALTER TABLE identity.link_tickets
    ADD COLUMN IF NOT EXISTS capabilities text[] NOT NULL DEFAULT '{}';

ALTER TABLE identity.upstream_flows
    ADD COLUMN IF NOT EXISTS capabilities text[] NOT NULL DEFAULT '{}';
