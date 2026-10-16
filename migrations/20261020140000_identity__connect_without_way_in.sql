-- Connecting a mailbox may leave it out of the person's ways in (a shared
-- mailbox such as support@): `identity.start_link` with `connect` and
-- `sign_in: false`. The choice rides on the ticket and on the round trip.
-- Every earlier ticket and flow added a way in, as the default says.
ALTER TABLE identity.link_tickets
    ADD COLUMN IF NOT EXISTS adds_way_in boolean NOT NULL DEFAULT true;

ALTER TABLE identity.upstream_flows
    ADD COLUMN IF NOT EXISTS adds_way_in boolean NOT NULL DEFAULT true;
