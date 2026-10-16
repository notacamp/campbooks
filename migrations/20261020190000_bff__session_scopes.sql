-- Fail fast rather than queue behind a long transaction while every request
-- that touches these tables waits behind the migration; the server's start
-- (and so the migration) is retried.
SET LOCAL lock_timeout = '5s';

-- The scopes a web session asked for at sign-in (space-separated). A session
-- that did not ask for one the BFF now asks for (a release added it) ends,
-- so the next sign-in asks for every scope; NULL (a session from before this
-- column) counts as asking for none. A nullable column: adding it rewrites
-- nothing. (campbooks-cloud#172)
ALTER TABLE bff.sessions ADD COLUMN scopes text;
