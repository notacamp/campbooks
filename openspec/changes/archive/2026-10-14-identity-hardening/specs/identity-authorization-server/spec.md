## ADDED Requirements

### Requirement: Authorization requests belong to their browser
`/oauth2/authorize` SHALL bind every request it stores to the browser that
made it (a random `__Host-campbooks_browser` cookie, `SameSite=Lax`, stored
as its SHA-256). Showing a request to the sign-in screen, using it in a
passkey ceremony, completing it after a sign-in and answering its consent
SHALL need that browser's cookie; in another browser the request SHALL be
unknown (404, or the sign-in lands on its plain return path with no code).

#### Scenario: A request id sent to someone else
- **WHEN** a sign-in screen link with a waiting request is opened in another browser and the person there signs in
- **THEN** the request is 404 there, no code is issued to its client, and the request still waits for its own browser

### Requirement: Refresh never outlives its sign-in
A refresh family SHALL expire no later than the issuer session it was issued
in.

#### Scenario: A sign-in near its end
- **WHEN** a code is redeemed from an issuer session that ends in an hour
- **THEN** its refresh token stops working when that session ends

### Requirement: Issuer endpoints are limited per address
`/oauth2/authorize` SHALL answer at most 60 requests per client address a
minute, `/oauth2/end_session` 30 and the web BFF's `/bff/login` 30; beyond
that 429 `rate_limited`. Calls made inside the server SHALL not be counted.

#### Scenario: Hammering authorize
- **WHEN** one address sends a 61st authorization request within a minute
- **THEN** the answer is 429, while another address is still served

### Requirement: A copied web session ends
The web BFF SHALL accept a rotated-away session cookie only during its
two-minute grace. Presented later, however many rotations later while the
session lives, once its successor has been used, the session SHALL end
everywhere it reaches: the web session, its refresh token at the issuer and
the issuer session; the log line SHALL name no session or person. While the
successor has never been presented (the response carrying it was lost), the
old cookie SHALL instead get a fresh one, and the undelivered successor SHALL
count as rotated away.

#### Scenario: An old cookie comes back
- **WHEN** a session's cookie rotated three minutes ago and the old value is presented
- **THEN** neither browser is signed in any more and the session's refresh token is revoked

#### Scenario: The new cookie never arrived
- **WHEN** a rotation's response is lost and the browser presents its old cookie three minutes later
- **THEN** it stays signed in with a fresh cookie, nothing is revoked, and the lost cookie, presented later, ends the session

#### Scenario: A copy two rotations old
- **WHEN** a session rotated twice while in use and a copy of its first cookie is presented
- **THEN** the session ends the same way

### Requirement: Names that read as Campbooks
Before deciding whether an app's name reads as Campbooks or Not A Camp, the
server SHALL decompose it for compatibility (NFKD), drop every mark
(general category M, in any script), split ligatures, and fold case,
spacing, look-alike punctuation and letters (Cyrillic, Greek, small
capitals, letters with a stroke, `rn` for `m`), so full-width, mathematical,
circled and accented variants count too; any other letter outside ASCII
SHALL count as any letter, but a match SHALL rest on at most two such
letters, so a name in another script is not refused for its length alone.

#### Scenario: A full-width name
- **WHEN** a workspace registers an app named `Ｃａｍｐｂｏｏｋｓ` or `Cámpbooks`
- **THEN** the answer is 400 `client_name_reserved`

#### Scenario: Letters with no decomposition
- **WHEN** a workspace registers an app named `Campbøøks` or `ᴄᴀᴍᴘʙᴏᴏᴋꜱ`
- **THEN** the answer is 400 `client_name_reserved`

#### Scenario: Other scripts
- **WHEN** a workspace registers an app named `メールアシスタントプロ`, `José` or `Françoise`
- **THEN** it is registered under that name

## MODIFIED Requirements

### Requirement: Workspace-bound tokens
A token issued after a consent in a workspace SHALL carry that workspace as
`workspace_id`. The in-process verifier SHALL keep that claim on a person's
token (an outside identity provider's user claims stay ignored), and
`Workspace<S>` SHALL answer 404 for any other workspace, whatever the
scopes; membership SHALL still be checked. Outside a workspace, such a token
SHALL list that workspace only (`workspaces.list`), and SHALL get 404 from
creating a workspace, accepting an invitation and every account-wide route
(`identity.*`), whatever scopes it holds. Inside its own workspace it SHALL
get 404 from managing the workspace's apps and adopting a held mailbox, and
the admin API SHALL answer it 404.

#### Scenario: Another workspace
- **WHEN** an agent allowed in Home calls a Work route with its token
- **THEN** the response is 404, while the same route of Home answers

#### Scenario: A bound token with identity scopes
- **WHEN** a token bound to Home that holds `identity:write` and `identity:clients` calls sign out everywhere, removes a way in, or lists Home's apps
- **THEN** each answers 404 and nothing changes

### Requirement: Client credentials
`/oauth2/token` with `grant_type=client_credentials` SHALL be accepted from a
workspace's confidential client only (anyone else `unauthorized_client`),
authenticated with `client_secret_basic` or `client_secret_post`, for at most
the scopes it was given and a third party may hold, checked again at issue
(`invalid_scope`). Its access token SHALL have `sub` = `azp` = the client id
and `workspace_id` = its workspace, and come with no refresh token and no ID
token.

#### Scenario: A workspace's export
- **WHEN** a workspace's confidential client asks for a token with its secret
- **THEN** the token is bound to its workspace and the API answers its workspace's routes and 404 for another's
