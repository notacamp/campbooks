-- Outbound changes bound to the request they were recorded for, with a MAC
-- under a key the database never holds (campbooks-cloud#227).
--
-- `connectors.request_changes` records a member's change only once the
-- conversations module confirms it (through the outbound-requests port, now
-- verifying that module's own MAC), and signs each row it records: the
-- workspace, row, stream, request, message, kind and labels. A stream's run
-- verifies it before it makes the change at the provider, so a row planted
-- or edited by SQL is skipped (`unverified`) and its message re-read from
-- the provider instead. A NULL MAC never verifies: a change still pending
-- when this runs was recorded by the previous version and is skipped the
-- same way (the provider's state then comes back to Campbooks).
SET LOCAL lock_timeout = '10s';

ALTER TABLE connectors.outbound_changes ADD COLUMN mac bytea;
