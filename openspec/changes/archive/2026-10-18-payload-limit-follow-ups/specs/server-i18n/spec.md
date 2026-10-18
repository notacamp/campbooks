## MODIFIED Requirements

### Requirement: Localized problem details
Every problem+json response SHALL carry a stable machine-readable `code`
(an English snake_case identifier that never changes with the locale) and
SHALL render `title` and `detail` in the request locale, with a
`Content-Language` header naming it. `type` SHALL NOT change with the
locale. When the request does not reach the localization layer, the problem
SHALL be complete in English. Rendering a problem SHALL wait at most two
seconds for the caller's saved locale, then use the `Accept-Language` locale,
so it never stretches a request past the server's time limit.

#### Scenario: A conflict in Portuguese
- **WHEN** a user whose saved locale is `pt-PT` tries to leave a workspace of which they are the last owner
- **THEN** the response is 409 with `code` `last_owner`, a Portuguese `title` and `detail`, and `Content-Language: pt-PT`

#### Scenario: Token errors before the user is known
- **WHEN** a request with an invalid token sends `Accept-Language: fr`
- **THEN** the response is 401 with `code` `invalid_token`, a French `title` and `detail`, and `Content-Language: fr`

#### Scenario: A 404 outside the API
- **WHEN** an unknown path is requested with `Accept-Language: es`
- **THEN** the problem has `code` `not_found`, a Spanish `title`, and `Content-Language: es-ES`

#### Scenario: A slow saved locale
- **WHEN** reading a user's saved locale takes longer than two seconds while a problem is rendered for a request sending `Accept-Language: pt-PT`
- **THEN** the problem is in Portuguese within those two seconds
