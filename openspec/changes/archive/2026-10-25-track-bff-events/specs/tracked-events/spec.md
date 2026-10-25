## ADDED Requirements

### Requirement: The web BFF's changes are tracked
Every web session the BFF starts or ends SHALL be tracked as an event of the
tracking plan, in the transaction that writes or deletes the session row:
a session started by a sign-in, a session ended (by logging out, by signing
in again, or because the BFF found its cookie copied, its scopes outdated,
its refresh refused, its tokens unreadable or its access token expired with
no refresh token), and the sessions of an account or of an identity
provider's session ended at once (back-channel logout, signing out
everywhere, a suspension), with how many ended. Ending a session that is
already gone SHALL track nothing. Events SHALL carry the session id, the
account id (when the session's subject is a Campbooks account id) and the
cause only: never a name, an address, a token, an identity provider's
session id, a user agent, a URL or an IP address. A request forwarded
through `/bff/api/*` or `/bff/admin/*` SHALL be tracked by the operation it
reaches, under that operation's id and as the person or administrator whose
session it is. Every mutating operation, handler and schedule of the BFF
SHALL have a Coverage row.

#### Scenario: Signing out of the web app
- **WHEN** a signed-in person posts `/bff/logout`
- **THEN** one `bff.web_session_ended` event is written with actor `anonymous`, operation `bff.logout`, the session id, the account id and cause `logout`, and logging out again tracks nothing

#### Scenario: Signing in on the web
- **WHEN** the callback finishes a sign-in in a browser that already had a session
- **THEN** a `bff.web_session_ended` event with cause `replaced` is written for the old session and a `bff.web_session_started` event for the new one, both in operation `bff.callback`

#### Scenario: A copied cookie
- **WHEN** a session token rotated away long ago is presented again
- **THEN** one `bff.web_session_ended` event is written with cause `copied_cookie`

#### Scenario: A call through the proxy
- **WHEN** a signed-in person posts to an API operation through `/bff/api/*`
- **THEN** the events that operation tracks carry its own operation id and actor `user` with the person's account id, not `bff.api_post`

#### Scenario: A suspension through the admin proxy
- **WHEN** an administrator suspends an account through `/bff/admin/*` and the account had a web session
- **THEN** `identity.account_suspended` is tracked in `identity.admin_suspend_account` as actor `admin`, and `bff.web_sessions_ended` with cause `account_suspended` and `sessions_ended` 1 as `system` on behalf of that administrator, with the same correlation
