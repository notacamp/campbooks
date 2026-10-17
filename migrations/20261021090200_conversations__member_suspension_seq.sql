-- A suspended member's rules pause, ordered by the account's counter
-- (campbooks-cloud#197).
--
-- `seq` is the change's place in the account's suspensions and lifts. Of two
-- changes for one member the higher `seq` wins; a change that carries none
-- (published before the counter existed) is older than any that does, and
-- two of those are ordered by `as_of`, as before. Null on existing rows.

-- Fail fast rather than queue behind a long transaction while every request
-- that touches these tables waits behind the migration; the server's start
-- (and so the migration) is retried.
SET LOCAL lock_timeout = '5s';

ALTER TABLE conversations.member_suspensions ADD COLUMN seq bigint;
