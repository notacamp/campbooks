## MODIFIED Requirements

### Requirement: API access through the BFF
A request to `/bff/api/<path>` with a live session SHALL be handled as
`/api/<path>` (same method, query, headers and body) by the API in the same
process, with the session's access token as `Authorization: Bearer`, and
without the browser's cookies, `Authorization` or CSRF headers. Without a live
session it SHALL answer 401 `session_required`. The access token SHALL be
refreshed with the refresh token shortly before it expires, never earlier
than the identity provider allows; concurrent requests of one session SHALL
cause one refresh. A refused refresh SHALL end the session. When the API
refuses the session's access token as invalid (401 `invalid_token`: its
sign-in ended, though the token has not expired), the BFF SHALL force one
refresh for that request, unless another request already replaced the token.
A refused refresh ends the session and the BFF answers 401
`session_required`. A refresh that works SHALL send a `GET` or `HEAD` again,
once, with the fresh token; any other method SHALL get the API's refusal
back, never sent twice, with the fresh token held for the next request. One
request SHALL cause at most one forced refresh.

#### Scenario: Token attached
- **WHEN** a signed-in browser requests `/bff/api/v1/<route>` of a scoped endpoint
- **THEN** the API sees the signed-in user as the principal and the browser never sees the token

#### Scenario: Refresh once under concurrency
- **WHEN** several requests of one session arrive while its access token is about to expire
- **THEN** exactly one refresh is made, all requests succeed, and the rotated refresh token is stored

#### Scenario: No session
- **WHEN** a browser without a live session requests `/bff/api/v1/system/version`
- **THEN** the response is 401 with `code` = `session_required`

#### Scenario: A refused token whose sign-in ended
- **WHEN** the API refuses a session's access token as `invalid_token` because the way in the session used was removed, or the person signed out everywhere on another device, and the refresh is refused too
- **THEN** that request gets 401 `session_required`, the session is gone and `/bff/session` says signed out, after one refresh attempt

#### Scenario: A refused token refreshed
- **WHEN** the API refuses a session's access token as `invalid_token` and the refresh works
- **THEN** a `GET` is sent again once with the fresh token and succeeds, while a `POST` gets the 401 back and the app's retry succeeds with the held fresh token
