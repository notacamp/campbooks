## ADDED Requirements

### Requirement: The redirect path sent to a provider is configurable
The `redirect_uri` Campbooks sends to a sign-in provider, in the authorization
request and again in the code exchange, SHALL be the instance's public URL plus
the provider's `CAMPBOOKS_<PROVIDER>__REDIRECT_PATH`, defaulting to
`/auth/callback/<provider>`. The path SHALL be absolute: it starts with a
single `/` and has no scheme, host, query, fragment or dot segments. A
configured path that is not SHALL stop the server at startup with an error
naming the setting. The route Campbooks serves SHALL stay
`/auth/callback/{provider}` whatever the setting says.

#### Scenario: Default
- **WHEN** `CAMPBOOKS_GOOGLE__REDIRECT_PATH` is unset or blank
- **THEN** the authorization request carries `redirect_uri=<public url>/auth/callback/google`

#### Scenario: Pre-registered path behind a proxy
- **WHEN** `CAMPBOOKS_GOOGLE__REDIRECT_PATH=/oauth/gmail/callback` and a proxy rewrites that path to `/auth/callback/google`
- **THEN** the authorization request and the token request both carry `redirect_uri=<public url>/oauth/gmail/callback`, and the sign-in completes

#### Scenario: Invalid path
- **WHEN** the setting is `https://evil.example.com/cb`, `//evil.example.com/cb`, `cb`, `/cb?x=1`, `/cb#f` or `/a/../b`
- **THEN** the server refuses to start with an error naming `CAMPBOOKS_GOOGLE__REDIRECT_PATH`
