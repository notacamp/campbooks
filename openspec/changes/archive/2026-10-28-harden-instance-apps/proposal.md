# Harden instance apps

Module: `identity` (the authorization server's clients and logout), with the
migrations. Tracking: https://github.com/notacamp/campbooks-cloud/issues/383
(follow-ups from the security review of instance apps, cloud #369).

## Why

None of these can be used from outside the instance, but each leaves a door
that should be shut before more apps rely on instance apps:

- the database guard on instance apps lists the columns it lets the runtime
  role change; a column added later and missed in that list would be writable
  by every request;
- an expired `id_token_hint` is accepted at logout however old it is, so a
  leaked ID token from a sibling app could end people's sessions for good;
- the CLI run with `CAMPBOOKS_ENV=development` against a production database
  stores loopback `http` redirects, which a production server would then
  honour;
- logging out of an instance app also ends the shared Campbooks browser
  session, and nothing said so.

## What Changes

- A new migration replaces the guard so the runtime role may change an
  instance app only if the whole row, minus `last_used_at`, is unchanged.
- An expired `id_token_hint` is accepted for 30 days after its `exp`; older is
  an invalid hint (400 `invalid_id_token_hint`, nothing ended).
- A server not running in development treats an instance app's stored
  loopback `http` redirect and post-logout URIs as not registered.
- Tests for the guard (an unchanged-row rule, and a workspace or dynamic
  client cannot be turned into an instance app), the hint's age and the
  loopback rule.
- The CLI help and the module docs say that logging out of an instance app
  ends the Campbooks browser session too.

## Impact

No new operation, job or event, so no tracking patch. Existing instance apps
with only `https` redirects are unaffected.
