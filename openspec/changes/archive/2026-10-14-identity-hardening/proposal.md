## Why
The security audit of identity (campbooks-cloud#117, batch 3) found ways a
stolen session or a lured browser could quietly keep or widen access: a new
way in added by a thief goes unnoticed; a copied web session cookie keeps
working beside the real one; there is no way to end every sign-in at once; a
passkey (which outlives every session) can be added on a session that is
merely recent; an authorization request id handed to another browser
completes there; refresh tokens outlive the sign-in behind them; the
authorization, sign-out, web sign-in and passkey sign-up endpoints can be
hammered from one address; an unverified address shows as a member's; and a
few delta-review notes from S4 (look-alike app names, bound tokens on
admin and workspace-app routes, the client-credentials scope ceiling, the
admin step-up window).

## What Changes
- **Way-in notices.** Every `identity.way_in_added` (but a sign-up's first)
  sends a notice by mail, in the person's language, to the account's verified
  address and the new way in's verified address, once, through a new kernel
  mailer (`CAMPBOOKS_MAIL__SMTP_URL` / `_FILE`, `CAMPBOOKS_MAIL__FROM`; off by
  default). Without one the notice is recorded in the account's audit trail
  and Ways to sign in marks ways in added in the last week.
- **Sign out everywhere** (`identity.sign_out_everywhere`, `identity:write`):
  ends every issuer session and refresh token of the account; the web BFF
  ends the account's web sessions on `identity.signed_out_everywhere`. A
  button on Ways to sign in.
- **A copied web session ends.** A rotated-away BFF cookie presented after
  its two-minute grace ends the session everywhere it reaches (BFF row,
  refresh token at the issuer, issuer session).
- **Passkeys need a sign-in from the last five minutes** to start a
  registration (`fresh_sign_in_required`); the web app signs in anew
  (`max_age=0`).
- **Authorization requests belong to their browser** (`__Host-campbooks_browser`).
- **Refresh families end with their issuer session.**
- **Rate limits** on `/oauth2/authorize`, `/oauth2/end_session` and
  `/bff/login` per address; passkey-only sign-up 5 per address an hour and
  200 per instance an hour (was 30 per instance).
- **Disabled accounts** never sign in with a passkey (tested).
- **Members' unverified addresses are not shown**; an unverified address does
  not count as a member's when inviting.
- Delta review: app names are folded with NFKD before the "reads as
  Campbooks" check; client credentials re-check the third-party scope
  ceiling at issue; bound tokens get 404 from the admin API and from a
  workspace's app and held-grant routes; the admin step-up window is one hour
  (was twelve).

## Capabilities
- New: `instance-mail`.
- Modified: `identity-accounts`, `identity-authorization-server`, `admin`,
  `workspaces`.

## Impact
- Migrations `identity__hardening` (notice ledger, `authorize_requests.browser_hash`,
  pending authorization requests dropped once) and `workspaces__verified_email`.
- New event `identity.signed_out_everywhere`; `identity.way_in_added` gains
  an optional `way_in` (still version 1).
- New operation `identity.sign_out_everywhere`; new error codes
  `fresh_sign_in_required`; `429` on `/oauth2/authorize`,
  `/oauth2/end_session`, `/bff/login`.
- New dependency `lettre` 0.11.23 (rustls, no native TLS); `quoted_printable`
  (0BSD) allowed by exception.
- New settings `CAMPBOOKS_MAIL__*`, documented; prod leaves them unset.
