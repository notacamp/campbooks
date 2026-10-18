## Why

#122 made suspension and sign-out-everywhere stop issued access tokens at
once, through the account's `tokens_not_before` epoch. Narrower revocations
still let that sign-in's access tokens run out their ten minutes
(campbooks-cloud#143):

- removing one way in (a provider link or a passkey);
- revoking one app's consent;
- `/oauth2/revoke` of a refresh token;
- ending one web session.

## What Changes

- **The sign-in on the token:** a person's access token from the in-process
  issuer already carries `sid` (its issuer session). It now also carries
  `fam`, the refresh family its sign-in holds when there is one. A token
  minted when a code is redeemed names the family that redemption creates,
  and rotation keeps it.
- **The check:** the kernel's `TokenRevocation` port now receives the whole
  principal, which gains its `sign_in` (`sid`, `fam`), and
  `TokenStanding` gains `sign_in_ended`. The identity adapter answers in one
  statement: the account (as before) plus three indexed lookups.
  - The session ended. This covers sign-out and the sessions ended when the
    way in they used is removed.
  - The family was revoked (revoke, reuse, sign-out).
  - A consent of the token's app that was live when the token was issued has
    been revoked since.
- **No cache, so no staleness:** a revocation takes effect on the next
  request in every process. The cost is the one round trip each request
  already paid for the account check. A new index,
  `consents (account_id, client_id)`, keeps the consent lookup cheap,
  because the live-consent unique index skips revoked rows.
- Every other sign-in of the person keeps working.

## Impact

- Spec: identity-authorization-server (Access and ID tokens; One sign-in's
  access tokens stop at once).
- Code:
  - kernel `auth` (`Principal::sign_in`, `SignIn`, `TokenStanding`, the
    port's signature, the `sid`/`fam` claims);
  - identity `token_revocation`, `domain::tokens` (`fam`, `rotate` returns
    the family) and `protocol::token`.
- Every `Principal` literal gains `sign_in: None`.
- Migration: an index on `identity.consents`, built concurrently.
