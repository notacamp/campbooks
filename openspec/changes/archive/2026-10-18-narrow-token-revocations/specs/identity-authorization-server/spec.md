## ADDED Requirements

### Requirement: One sign-in's access tokens stop at once
The API SHALL also refuse a person's access token of the in-process issuer,
on every verification and through the same `TokenRevocation` port, once its
own sign-in has ended: its issuer session (`sid`) ended (signed out, or
ended because the way in it used was removed), its refresh family (`fam`)
revoked (`/oauth2/revoke`, reuse, sign-out), or a consent of its app live
when it was issued revoked since. The token gets the answer of any invalid
token. Every other sign-in of the same person SHALL keep working. The check
SHALL cost the single statement the account check already needs, with no
cache, so a revocation takes effect on the next request in every process. A
sign-in named with something that is no id SHALL stand for nothing.

#### Scenario: One session signed out
- **WHEN** a person signed in in two browsers signs out of the first
- **THEN** the first browser's access token gets 401 `invalid_token` at once, and the second's still works and refreshes

#### Scenario: One refresh token revoked
- **WHEN** the first sign-in's refresh token is revoked at `/oauth2/revoke`
- **THEN** that sign-in's access token gets 401 at once, and the other sign-in's tokens, refreshed ones included, still work

#### Scenario: A way in removed
- **WHEN** a person removes the way in another browser signed in with
- **THEN** that browser's access token gets 401 at once, and the person's own sign-in still works

#### Scenario: An app's consent revoked
- **WHEN** a person revokes an app's consent
- **THEN** the app's access token gets 401 `invalid_token` at once, and the person's own tokens still work

## MODIFIED Requirements

### Requirement: Access and ID tokens
Access tokens SHALL be JWTs with header `typ` = `at+jwt`, signed with an
Ed25519 key published in `/oauth2/jwks`, carrying `iss`, `sub`, `aud` (the
API's resource URI, `<public origin>/api`), `client_id`, `azp`, `scope`,
`iat`, `nbf`, `exp` (ten minutes after `iat`), `jti`, `auth_time` for a
person's token, `sid` (the issuer session) when it was issued in one, `fam`
(the refresh family) when its sign-in holds a refresh token, kept across
rotation, `workspace_id` when the token is bound to a workspace, and
the person's address and name when `email` and `profile` were granted. ID
tokens SHALL be signed the same way with `typ` = `JWT`, for the client,
carrying the request's `nonce` and the session's authentication methods
(`amr`, RFC 8176): `["fed"]` for a provider sign-in, `["hwk", "user"]` for a
device-bound passkey and `["swk", "user"]` for a synced one. The API SHALL validate access tokens with the
same verifier it uses for any issuer, reading the keys in-process, SHALL
expect the resource URI as `aud`, and SHALL accept only `typ` = `at+jwt`
from the in-process issuer, so an ID token never passes as an access token.

#### Scenario: Token accepted by the API
- **WHEN** the web client calls the API with an access token from the issuer
- **THEN** the API accepts it and the handler sees the account id as the principal's subject

#### Scenario: An ID token is not an access token
- **WHEN** a client calls the API with an ID token of the in-process issuer whose `aud` matches the API's
- **THEN** the response is 401 with `code` = `invalid_token`

#### Scenario: A passkey sign-in's amr
- **WHEN** the web client redeems a code from a sign-in with a device-bound passkey
- **THEN** the ID token's `amr` is `["hwk", "user"]`, while one from a Google sign-in has `["fed"]`
