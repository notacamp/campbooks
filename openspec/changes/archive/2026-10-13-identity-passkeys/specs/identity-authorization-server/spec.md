## MODIFIED Requirements

### Requirement: Access and ID tokens
Access tokens SHALL be JWTs with header `typ` = `at+jwt`, signed with an
Ed25519 key published in `/oauth2/jwks`, carrying `iss`, `sub`, `aud` (the
configured audience), `client_id`, `azp`, `scope`, `iat`, `nbf`, `exp` (ten
minutes after `iat`), `jti` and `auth_time`, and the person's address and
name when `email` and `profile` were granted. ID tokens SHALL be signed the
same way with `typ` = `JWT`, for the client, carrying the request's `nonce`
and the session's authentication methods (`amr`, RFC 8176): `["fed"]` for a
provider sign-in, `["hwk", "user"]` for a device-bound passkey and
`["swk", "user"]` for a synced one.
The API SHALL validate access tokens with the same verifier it uses for any
issuer, reading the keys in-process, and SHALL accept only `typ` = `at+jwt`
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
