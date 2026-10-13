## ADDED Requirements

### Requirement: Passkeys are ways in
A passkey SHALL be a discoverable WebAuthn credential registered with user
verification, belonging to one account. Its credential id SHALL be unique
across the instance. An account's ways in SHALL be its provider identities
and its passkeys, and the last one SHALL never be removed. The relying-party
id SHALL be the host of the public URL and the only accepted origin that
URL's origin; an instance whose public URL is plain http on a host other than
`localhost`, or an IP address, SHALL offer no passkeys (its passkey endpoints
answer 404).

#### Scenario: Passkeys on offer
- **WHEN** a sign-in request is read on an https instance with Google, Microsoft and Zoho configured
- **THEN** it lists `["google", "microsoft", "zoho", "passkey"]`

### Requirement: Adding a passkey while signed in
`identity.start_passkey_registration` (`identity:write`) SHALL need a
sign-in within the last ten minutes and SHALL return creation options: a
single-use challenge valid five minutes and bound to the account, the
account id as user handle, `residentKey` and `userVerification` required,
attestation `none`, algorithms EdDSA (-8), ES256 (-7) and RS256 (-257), and
the account's passkeys as `excludeCredentials`. `identity.register_passkey`
(`identity:write`, recent sign-in) SHALL take that challenge from the new
credential's client data, spending it whatever the outcome, verify the
credential, store it with its name (1 to 64 characters, default "Passkey"),
audit it and publish `identity.way_in_added` with method `passkey`. A
challenge started by another account, spent or expired SHALL be 409
`passkey_expired`; a credential that does not verify SHALL be 403
`passkey_not_accepted`; a credential id already registered SHALL be 409
`passkey_already_registered`.

#### Scenario: Add a passkey
- **WHEN** a person who signed in five minutes ago adds a passkey and approves it on their device
- **THEN** it is listed among their passkeys and signing in with it later opens the same account

#### Scenario: Stale sign-in
- **WHEN** the person last signed in an hour ago and starts a registration
- **THEN** the response is 403 with `code` = `step_up_required`

#### Scenario: Another account's challenge
- **WHEN** a credential made for one account's registration challenge is submitted by another account
- **THEN** the response is 409 with `code` = `passkey_expired` and nothing is stored

### Requirement: Signing in with a passkey
`POST /auth/passkey/sign-in/options` (CSRF header) SHALL return request
options for any of the site's passkeys (no `allowCredentials`, user
verification required) and set a `SameSite=Strict` ceremony cookie binding a
single-use, five-minute challenge to the browser. `POST /auth/passkey/sign-in`
SHALL take that challenge only for the browser holding the cookie, find the
passkey by credential id, require the user handle to be its account, verify
the assertion, and start an issuer session signed in with that passkey,
then answer where to go (the client's redirect URI with a code when an
authorization request waits). Both SHALL be rate-limited per client address.
A passkey no account has (removed, or never registered), a disabled
account's, or an assertion that does not verify SHALL be 403
`passkey_not_accepted`, and no session SHALL start. A missing, spent,
expired or foreign ceremony SHALL be 409 `passkey_expired`.

#### Scenario: Sign in from another device's browser
- **WHEN** a person with a synced passkey signs in with it on a new browser
- **THEN** the web client gets a code for the same account, and the ID token's `amr` is `["swk", "user"]`

#### Scenario: Replay
- **WHEN** an assertion that already signed someone in is posted again
- **THEN** the response is 409 with `code` = `passkey_expired`

#### Scenario: Relayed from another origin
- **WHEN** an assertion whose client data names another origin is posted with a valid ceremony
- **THEN** the response is 403 with `code` = `passkey_not_accepted` and no session starts

### Requirement: Signing up with a passkey alone
`POST /auth/passkey/sign-up/options` (CSRF) SHALL take a display name (1 to
64 printable characters, else 400 `display_name_required`) and return
creation options for a new account id, bound to the browser like a sign-in
ceremony. `POST /auth/passkey/sign-up` SHALL verify the new credential and,
in one transaction, create the account with that name and no address, its
passkey and an issuer session, publish `identity.account_created` (provider
`passkey`) and `identity.way_in_added`, and answer where to go. Both SHALL
be refused with 403 `signup_invite_only` on an invite-only instance, keeping
nothing, because a passkey proves no address an invitation could match; the
sign-in request SHALL say whether passkey sign-up is open
(`passkey_sign_up`).

#### Scenario: A new person with a passkey
- **WHEN** someone signs up with a passkey and the name "Ana Lima" on an open instance
- **THEN** an account exists with that name and no address, and the web client gets a code for it

#### Scenario: Invite only
- **WHEN** someone starts a passkey sign-up on an invite-only instance
- **THEN** the response is 403 with `code` = `signup_invite_only`, and no challenge or account is kept

### Requirement: WebAuthn verification
Registration SHALL follow WebAuthn Level 3 §7.1 and sign-in §7.2 for this
profile:
- the credential type is `public-key` and its `id` equals `rawId`;
- the client data has the ceremony's `type`, the challenge, the exact
  origin, and no `crossOrigin` true and no `topOrigin`;
- `rpIdHash` is SHA-256 of the relying-party id, user presence and user
  verification are set, and BS is set only with BE;
- a registration's attestation object is well-formed CBOR with a registered
  `fmt` (`none` with an empty statement; other formats' statements are not
  trusted);
- its credential id is at most 1023 bytes and equals `rawId`;
- its COSE key states one of the three algorithms, matching its key type and
  curve, with no private part, and RSA keys are at least 2048 bits;
- an assertion carries no attested credential data, keeps the BE it was
  registered with, and is signed over `authData || SHA-256(clientDataJSON)`
  by the stored key.

A signature counter that does not increase while either it or the stored
one is non-zero SHALL be audited (`passkey_sign_count_regressed`), not
refused, and the stored counter SHALL never decrease.

#### Scenario: Every algorithm
- **WHEN** passkeys with EdDSA, ES256 and RS256 keys are registered and used
- **THEN** each registers and signs in

#### Scenario: No user verification
- **WHEN** an assertion's authenticator data has user presence but not user verification
- **THEN** it is refused

#### Scenario: A cloned authenticator's counter
- **WHEN** a passkey whose stored counter is 10 signs in reporting 10
- **THEN** the sign-in succeeds, the stored counter stays 10, and the audit trail records the regression

### Requirement: Renaming and removing a passkey
`identity.list_passkeys` (`identity:read`) SHALL list the caller's passkeys
(name, whether synced, added, last used; never keys).
`identity.rename_passkey` (`identity:write`) SHALL rename one (1 to 64
characters, else 400 `passkey_name_required`). `identity.remove_passkey`
(`identity:write`) SHALL need a recent sign-in, lock the account, refuse with
409 `last_way_in` when it is the account's only way in, end every session
signed in with it and revoke their refresh tokens, audit it and publish
`identity.way_in_removed`. Another account's passkey SHALL be 404 for all
three.

#### Scenario: Remove the only passkey of a passkey-only account
- **WHEN** a person whose only way in is a passkey removes it
- **THEN** the response is 409 with `code` = `last_way_in`

#### Scenario: Signed out with it
- **WHEN** a passkey is removed while a browser is signed in with it
- **THEN** that session's refresh token is refused, and the passkey no longer signs in

### Requirement: Passkeys section
Settings → Ways to sign in SHALL have a Passkeys section below the provider
ways in: the person's passkeys (synced or on one device, added, last used),
Rename in place, Remove behind an inline confirmation (`last_way_in`
explained in place), and "Add a passkey" (`N` then `P`), naming the new
passkey after the device. A refusal for a stale sign-in SHALL offer "Sign in
again" through `/bff/login?max_age=600`. A browser without passkeys SHALL be
told so instead of offered Add. Every string SHALL be in en, pt-PT, es-ES
and fr, and the section usable at 375px.

#### Scenario: Add needs a fresh sign-in
- **WHEN** the person presses Add a passkey an hour after signing in
- **THEN** the section says to sign in again first and offers it, coming back to the page
