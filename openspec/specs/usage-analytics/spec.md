# usage-analytics Specification

## Purpose
How an operator learns which screens of the app people use: the app names
the screen's route template to its own server, which relays it to the
operator's Matomo with no IP, no identity and a visitor id that changes
every day. Off unless configured; no analytics script runs in the app.

## Requirements

### Requirement: Screen view relay
`POST /api/v1/system/screen-views` SHALL accept `{ "route": <template> }`
from a caller holding `system:read` through a token not bound to a
workspace, acting for an account, and SHALL answer 204 without waiting for
any outside service. When `CAMPBOOKS_ANALYTICS__MATOMO_URL`,
`CAMPBOOKS_ANALYTICS__MATOMO_SITE_ID` and
`CAMPBOOKS_ANALYTICS__MATOMO_VISITOR_KEY` are all set, the view SHALL join a
bounded in-memory queue that one background worker sends to
`<MATOMO_URL>/matomo.php` with a short timeout; otherwise nothing SHALL be
queued or sent. A full queue or a failed send SHALL drop the view, never
retry it and never answer the caller with an error; nothing SHALL be stored.

#### Scenario: Relayed
- **WHEN** a signed-in app posts the route `/w/:workspaceId/inbox` and analytics is set
- **THEN** the answer is 204 and Matomo receives one tracking request for that route

#### Scenario: Off
- **WHEN** any of the three analytics settings is unset or blank
- **THEN** the answer is 204 and no request leaves the server

#### Scenario: Matomo down or slow
- **WHEN** Matomo answers 500 or the queue is full
- **THEN** the answer is still 204 and the view is dropped, not retried

#### Scenario: No token
- **WHEN** a view is posted without an access token
- **THEN** the answer is 401 and nothing is sent

### Requirement: Only route templates leave the server
The relay SHALL accept as `route` only `/`, or at most 8 segments of at
most 32 bytes (120 bytes in all), each a literal of lowercase letters, `-`
and `_` starting with a letter or a parameter `:name` of letters starting
lowercase, whether analytics is on or off. Anything else SHALL be answered
400 `invalid_route` and never forwarded: no id, UUID or number, no query
string, fragment, title or address.

#### Scenario: A concrete path
- **WHEN** the app posts `/w/0192a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b/inbox`, `/people/42`, `/search?q=boiler`, `/inbox#message-3` or `Inbox – Campbooks`
- **THEN** the answer is 400 `invalid_route` and Matomo receives nothing

### Requirement: Matomo learns the screen, not the person
Each tracking request SHALL carry exactly `idsite`, `rec=1`, `apiv=1`,
`url` (the instance's public URL followed by the route), `action_name` (the
route), `cid`, `send_image=0`, and, when the request has them, `ua` (its
`User-Agent`, capped) and `lang` (its `Accept-Language` reduced to a shipped
locale). `cid`, the forced visitor id, SHALL be the first 16 hex characters
of HMAC-SHA256 under the visitor key of the account id, `:` and the UTC date
(`YYYY-MM-DD`). No IP (`cip`), `token_auth`, user id (`uid`), `_id`,
workspace or custom dimension SHALL be sent. The visitor key SHALL never be logged and SHALL be redacted
wherever the configuration is printed.

#### Scenario: The same person, two days
- **WHEN** one account views screens on 2026-10-18 and on 2026-10-19
- **THEN** its views share one visitor id on each day, the two days' ids differ, and neither is the account id

#### Scenario: Two people
- **WHEN** two accounts view a screen on the same day
- **THEN** their visitor ids differ

### Requirement: The app reports the screen it shows
After each navigation to another path, the app SHALL post the shown route's
template, with parameters written `:name` and no trailing slash, through
the API client every call already uses (the web BFF or the desktop bridge),
never the address, an id, the search, the fragment or the page title, and
SHALL load no analytics script. It SHALL send nothing while
`navigator.doNotTrack` is `"1"` or `navigator.globalPrivacyControl` is
`true`. A send SHALL never be awaited by the navigation, and a failed send
SHALL be dropped, never retried.

#### Scenario: Navigating
- **WHEN** the person opens a conversation at `/w/<id>/inbox/primary/<id>`
- **THEN** the app posts `/w/:workspaceId/inbox/:stream/:conversationId`

#### Scenario: Do Not Track
- **WHEN** the browser sends Do Not Track or Global Privacy Control
- **THEN** the app posts no screen view
