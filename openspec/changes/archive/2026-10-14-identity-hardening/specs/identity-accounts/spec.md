## ADDED Requirements

### Requirement: Notice of a new way in
Every `identity.way_in_added` but a sign-up's first way in SHALL be told to
the account. When the instance sends mail, a notice SHALL go to the account's
primary address and to the new provider way in's address, each only when
verified and once per address, in the language the person chose (English
otherwise), naming what was added and when, with one link (the instance's
public URL) and no images or remote content. A delivery SHALL never send a
notice twice to the same address, SHALL be retried while the mail server is
unreachable, and SHALL leave an audit row with ids and counts only. Without
mail, the audit row SHALL record `no_mail_transport`, and Ways to sign in SHALL
mark the ways in added in the last seven days, never the one the account was
made with (its sign-up provider account or passkey, which `PasskeyView` says
with `signed_up_with`).

#### Scenario: Zoho added
- **WHEN** a person who signed up with Google (verified) adds a Zoho account (verified) on an instance that sends mail
- **THEN** both addresses get one notice, in the person's language, saying a Zoho account was added and when

#### Scenario: Delivered twice
- **WHEN** the same event is delivered again after the notices went
- **THEN** nothing more is sent

#### Scenario: Mail server down
- **WHEN** the mail server is unreachable at the first attempt and back later
- **THEN** the delivery is retried, the notices go, and the job's recorded error names no address

#### Scenario: Unverified address
- **WHEN** a passkey is added to an account whose primary address is not verified
- **THEN** no notice is sent

### Requirement: Signing out everywhere
`identity.sign_out_everywhere` (`identity:write`, `POST
/api/v1/identity/sign-out-everywhere`) SHALL end every live issuer session of
the caller's account and revoke every refresh token it holds, for every
client, record it in the audit trail and publish
`identity.signed_out_everywhere`; the web BFF SHALL end the account's web
sessions on it. It SHALL need no recent sign-in. A token bound to a workspace
SHALL get 404 and end nothing. Ways to sign in SHALL offer it behind an
inline confirmation, then sign the browser out.

#### Scenario: A stolen session
- **WHEN** a person signs out everywhere while another browser holds a session and a refresh token of their account
- **THEN** that refresh token fails with `invalid_grant`, that browser's next authorization shows the sign-in screen, and its web session ends

## MODIFIED Requirements

### Requirement: Adding a passkey while signed in
`identity.start_passkey_registration` (`identity:write`) SHALL need a
sign-in within the last five minutes (else 403 `fresh_sign_in_required`, and
the web app signs in again with `max_age=0`) and SHALL return creation
options: a single-use challenge valid five minutes and bound to the account,
the account id as user handle, `residentKey` and `userVerification`
required, attestation `none`, algorithms EdDSA (-8), ES256 (-7) and RS256
(-257), and the account's passkeys as `excludeCredentials`.
`identity.register_passkey` (`identity:write`, a sign-in within the last ten
minutes) SHALL take that challenge from the new credential's client data,
spending it whatever the outcome, verify the credential, store it with its
name (1 to 64 characters, default "Passkey"), audit it and publish
`identity.way_in_added` with method `passkey` and the passkey. A challenge
started by another account, spent or expired SHALL be 409
`passkey_expired`; a credential that does not verify SHALL be 403
`passkey_not_accepted`; a credential id already registered SHALL be 409
`passkey_already_registered`.

#### Scenario: Add a passkey
- **WHEN** a person who signed in two minutes ago adds a passkey and approves it on their device
- **THEN** it is listed among their passkeys and signing in with it later opens the same account

#### Scenario: Stale sign-in
- **WHEN** the person last signed in an hour ago and starts a registration
- **THEN** the response is 403 with `code` = `fresh_sign_in_required`

#### Scenario: Not fresh enough
- **WHEN** the person last signed in seven minutes ago and starts a registration
- **THEN** the response is 403 with `code` = `fresh_sign_in_required`, though removing a passkey still works

#### Scenario: Another account's challenge
- **WHEN** a credential made for one account's registration challenge is submitted by another account
- **THEN** the response is 409 with `code` = `passkey_expired` and nothing is stored

### Requirement: Passkey ceremonies store little and wait in small numbers
The ceremony endpoints SHALL read at most 16 KiB of body (else 400
`invalid_body`). A `request` they are given SHALL be at most 64 URL-safe
characters and name a waiting authorization request of the same browser, or
the answer SHALL be 404 and nothing stored. One client address SHALL start
at most ten ceremonies, and one account at most ten registrations, per five
minutes (a challenge's lifetime); one address SHALL create at most 5
passkey-only accounts an hour, and the instance at most 200; beyond that the
answer SHALL be 429 `rate_limited`. A browser SHALL keep one ceremony cookie
for all its waiting ceremonies, so a ceremony started later (the passkey
dialog after autofill) leaves earlier ones valid.

#### Scenario: An unknown or oversized request id
- **WHEN** sign-in options are asked for with a `request` that is unknown, expired, another browser's, longer than 64 characters or in a 20 KB body
- **THEN** the answer is 404 (400 for the body) and no challenge is stored

#### Scenario: Autofill, then the dialog
- **WHEN** the sign-in screen starts an autofill ceremony and the person then presses `P`
- **THEN** both challenges are valid in that browser, and the autofill pick still signs in

#### Scenario: Passkey sign-ups from one address
- **WHEN** one address creates a sixth passkey-only account within the hour
- **THEN** the answer is 429, while another address can still sign up

### Requirement: Ways to sign in screen
Settings SHALL have a Ways to sign in page listing the person's provider ways
in, with Remove behind an inline confirmation (and `last_way_in` explained
in place), Add for each configured provider, Reconnect for this workspace's
connections of theirs that need re-authorisation, a "Sign in again" offer
when an action needs a recent sign-in, further sections other slices
contribute (passkeys), a notice and a "New" mark when a way in (other than
the sign-up's) was added in the last seven days, and Sign out everywhere
behind an inline confirmation. The sign-in screen SHALL offer every provider
the server lists. Both SHALL be in en, pt-PT, es-ES and fr, keyboard-operable
with visible hints, and usable at 375px.

#### Scenario: Remove with confirmation
- **WHEN** the person presses Remove on a way in
- **THEN** an inline confirmation names the provider and address and says the mailbox stays connected, and nothing is removed until they confirm

#### Scenario: Microsoft and Zoho offered
- **WHEN** the server lists `google`, `microsoft` and `zoho` for a sign-in request
- **THEN** the sign-in screen shows the three, in that order, with `1`, `2` and `3`

#### Scenario: Added this week
- **WHEN** a Zoho account was added two days ago
- **THEN** its row is marked New and the page says to remove it and sign out everywhere if it wasn't the person
