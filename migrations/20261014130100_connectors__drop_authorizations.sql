-- A connection's consent now runs through the identity module's front door
-- (ADR 0014, slice S2): its round trip is an `identity.upstream_flows` row,
-- and the grant arrives through the grant-intake port. The consents
-- `connectors.start_connection` used to keep are no longer written or read.
-- Any left in flight expired within ten minutes of being started.
DROP TABLE IF EXISTS connectors.authorizations;
