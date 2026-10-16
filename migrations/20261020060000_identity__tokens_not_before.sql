-- The revocation epoch of an account's access tokens (campbooks-cloud#122).
-- Suspending an account or signing out everywhere sets it to that instant,
-- in the same transaction that ends the account's sessions; unsuspending
-- leaves it. The API refuses a person's access token unless its `iat` falls
-- in a later second, and the token endpoint mints nothing for a sign-in
-- that is not newer. NULL: nothing revoked yet.
--
-- A nullable column with no default: a catalog-only change, no rewrite.
SET LOCAL lock_timeout = '5s';

ALTER TABLE identity.accounts ADD COLUMN tokens_not_before timestamptz;
