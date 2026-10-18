-- no-transaction
-- An account's consents to one app, revoked ones included: read on every
-- verification of that app's access tokens, whether a consent live when a
-- token was issued has been revoked since (campbooks-cloud#143). The live
-- consents' unique index covers only those not revoked. Built
-- CONCURRENTLY, one statement per migration.
CREATE INDEX CONCURRENTLY IF NOT EXISTS consents_account_client_idx
    ON identity.consents (account_id, client_id);
