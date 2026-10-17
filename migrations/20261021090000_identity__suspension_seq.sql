-- Suspensions are ordered by a counter, not the clock (campbooks-cloud#197).
--
-- `suspension_seq` counts an account's suspensions and lifts: every change
-- bumps it by one, under the account's row lock, and the events carry it,
-- so consumers order two changes by it whatever the clock did between them
-- (a clock stepped back could make a lift look older than its suspension).
-- An even count is an active account, an odd one a suspended account.

-- Fail fast rather than queue behind a long transaction while every request
-- that touches these tables waits behind the migration; the server's start
-- (and so the migration) is retried.
SET LOCAL lock_timeout = '5s';

-- A constant default: no table rewrite.
ALTER TABLE identity.accounts
    ADD COLUMN suspension_seq bigint NOT NULL DEFAULT 0 CHECK (suspension_seq >= 0);

-- Accounts suspended today start at 1, consistent with their state.
UPDATE identity.accounts SET suspension_seq = 1 WHERE disabled_at IS NOT NULL;
