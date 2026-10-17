-- The newest suspension change this module announced for each user
-- (campbooks-cloud#197), kept on the user's row and changed only under its
-- lock, like joining and leaving a workspace: a workspace the user joins
-- while the change is announced is told it on joining, so no membership
-- misses a suspension or its lift.
--
-- `suspension_seq` is the account's counter from identity (null for an event
-- published before it existed), `suspension_at` when the change took effect;
-- both null until a change reaches this module (nothing to announce then).

-- Fail fast rather than queue behind a long transaction while every request
-- that touches these tables waits behind the migration; the server's start
-- (and so the migration) is retried.
SET LOCAL lock_timeout = '5s';

-- Constant defaults: no table rewrite. Nothing to backfill: identity's state
-- is not readable here, and a user already suspended cannot join anything.
ALTER TABLE workspaces.users
    ADD COLUMN suspended boolean NOT NULL DEFAULT false,
    ADD COLUMN suspension_seq bigint,
    ADD COLUMN suspension_at timestamptz,
    ADD CONSTRAINT users_suspension_known
        CHECK (suspension_at IS NOT NULL OR (suspension_seq IS NULL AND NOT suspended));
