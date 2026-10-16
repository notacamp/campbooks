## Why

A web session keeps the scopes it asked for at sign-in: refreshing never
widens them. When a release adds scopes (a new module, such as the assistant's
`assistant:read` and `assistant:write` in #485), every session signed in
before it gets 403 `insufficient_scope` for the new features until the person
signs out. The UI shows nothing at all (Home's offer) or an error that says
nothing useful ("couldn't load your answers"). Seen in production on
2026-10-16 (campbooks-cloud#172).

## What Changes

- Each web session records the scopes its sign-in asked for when it started
  (`bff.login_transactions.scopes` → `bff.sessions.scopes`), so a sign-in that
  spans a deploy keeps what it really asked for.
- A session that did not ask for every scope the BFF asks for now ends like an
  expired session. Its refresh token is revoked; the identity provider's own
  session is left alone. The app's existing handling (401 `session_required`
  → `/bff/login`, back to the same page) signs in again and comes back with
  every scope, straight through when the provider's session is still live.
- Signing out looks the session up without the scope check, so an outdated
  session still signs out in full (the provider's session too).
- What is compared is what the session *asked for*, never what was granted,
  so a provider that grants less cannot cause a sign-in loop: right after a
  sign-in the two always match.
- Sessions from before this change (no recorded scopes) end once, on their
  next request.

## Impact

- `crates/server/src/bff` (session lookup, sign-in, store), one migration
  `20261020200000_bff__session_scopes.sql` (two nullable columns, no rewrite).
- No client change, no API change.
- Self-hosters: everyone signed in to the web app signs in once more after
  upgrading. Nothing else to do.
