## Why
An instance owner who wants only their own people in has no way to say so.
`CAMPBOOKS_SIGNUP__POLICY=invite_only` stops new accounts without an
invitation, but any account that already exists can still sign in, and there
is no way to name the addresses or domains that may.

## What Changes
- New setting `CAMPBOOKS_SIGNUP__ALLOWED`: comma-separated entries, an entry
  with `@` an exact address, any other a domain matching `*@domain` exactly
  (no subdomains); case-insensitive; blanks ignored. Unset or empty means no
  restriction. A malformed entry stops startup.
- When set it is checked, on top of the signup policy (an invitation does
  not bypass it), at every door that creates an account or issues a session
  or tokens: the provider sign-in (verified address), account creation (again
  at creation), linking a way in, passkey sign-in, passkey sign-up and every
  token exchange (authorization code and refresh token).
- Refusals are `sign_in_not_allowed`: `/sign-in?error=sign_in_not_allowed`
  from a callback, 403 from the passkey and new-account endpoints, and
  `link_error=sign_in_not_allowed` from a signed-in round trip, with copy in
  the four languages. They say nothing about whether an account exists, and
  nothing is kept.

## Capabilities
- Modified: `identity-accounts`.

## Impact
Module: `identity` (and the kernel's `SignupConfig`). No migration, no new
event, no new operation. New error code `sign_in_not_allowed` in the
identity catalogue and the web client's identity namespace.
Tracking issue: none recorded yet.

## Non-goals
- Per-workspace allow-lists, subdomain wildcards, or a UI to edit the list.
- Cutting a live access token short: those live out their ten minutes.
- Revoking sessions or refresh tokens when the list tightens (they are
  refused at their next exchange instead).
