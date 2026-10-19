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
  `fam`, the refresh family its sign-in holds when there is one, and `cns`,
  the consent an app that needs one was minted under. A token minted when a
  code is redeemed names the family that redemption creates, and rotation
  keeps it.
- **The check:** the kernel's `TokenRevocation` port now receives the whole
  principal, which gains its `sign_in` (`sid`, `fam`), and
  `TokenStanding` gains `sign_in_ended`. The identity adapter answers in one
  statement: the account (as before) plus three indexed lookups.
  - The session ended. This covers sign-out and the sessions ended when the
    way in they used is removed.
  - The family was revoked (revoke, reuse, sign-out).
  - The consent it was minted under (`cns`) has been revoked.
  - Each lookup is by primary key. The first version matched any consent of
    the app revoked around the token's `iat`. That refused a token redeemed
    right after a re-consent, because a re-consent revoked the previous row.
- **Re-consent keeps an equal consent:** an app asked again for exactly what
  its live consent holds (the same workspace, member and scopes) keeps that
  consent. Native and agent apps are asked on every sign-in, so its tokens
  on the person's other devices go on. A different grant still replaces the
  consent.
- **No cache, so no staleness:** a revocation takes effect on the next
  request in every process. The cost is the one round trip each request
  already paid for the account check. Every lookup is by primary key, so no
  new index is needed. A token minted before these claims existed is judged
  by its account alone, for the rest of its ten minutes.
- Every other sign-in of the person keeps working.
- **The web app signs in again at once:** the BFF holds an access token
  until 30 s before it expires. Once that token's sign-in ended, the API
  refused it while the web app kept saying "signed in", for up to ten
  minutes. Now a 401 `invalid_token` from the API makes the BFF force one
  refresh. If the refresh is refused, the session ends and the app gets
  `session_required`, so it signs in again. If the refresh works, a read is
  sent again once; a write gets the refusal back and is never sent twice.

## Impact

- Spec: identity-authorization-server (Access and ID tokens; One sign-in's
  access tokens stop at once); web-bff (API access through the BFF).
- Code:
  - kernel `auth` (`Principal::sign_in`, `SignIn`, `TokenStanding`, the
    port's signature, the `sid`/`fam` claims);
  - identity `token_revocation`, `domain::tokens` (`fam`, `rotate` returns
    the family) and `protocol::token`.
  - the server's BFF (`forward`, `Inner::refresh`).
- Every `Principal` literal gains `sign_in: None`.
- No migration.
