## ADDED Requirements

### Requirement: Sign-in allow-list
`CAMPBOOKS_SIGNUP__ALLOWED` SHALL be a comma-separated list where an entry
containing `@` is one exact address and any other entry is a domain matching
`*@domain` exactly (no subdomains), compared case-insensitively, entries
trimmed and empty ones ignored. Unset or empty SHALL mean no restriction. A
malformed entry SHALL stop startup with an error naming the setting and the
entry's position, never its text. When set, on top of the signup policy and
whatever invitations exist, the issuer SHALL refuse with `sign_in_not_allowed`
(and keep nothing) at: a provider sign-in whose provider-verified address is
not listed, for known and unknown accounts alike, before looking for an
account; creating an account from a pending sign-up whose address is no
longer listed; adding a way in whose verified address is not listed
(`link_error=sign_in_not_allowed`); a passkey sign-in for an account with no
way in whose address was verified and is listed; a passkey sign-up; and any
token exchange (`authorization_code` or `refresh_token`) for an account with
no such way in, answering `invalid_grant`. The sign-in request SHALL report
`passkey_sign_up` false. The refusal SHALL NOT reveal whether an account
exists.

#### Scenario: Listed domain and address
- **WHEN** the list is `example.com` or `ana@example.com` and Ana signs in with a provider-verified `ana@example.com`
- **THEN** she signs in or creates her account as usual

#### Scenario: Not listed, with an invitation
- **WHEN** an `invite_only` instance lists only `bo@example.com` and Ana has a pending invitation
- **THEN** her round trip ends at `/sign-in?error=sign_in_not_allowed` and no account, pending sign-up, token or held grant is kept

#### Scenario: Existing account off the list
- **WHEN** the list excludes the address of an existing account and its owner signs in with the provider
- **THEN** the round trip ends at `/sign-in?error=sign_in_not_allowed`, the same as for someone with no account, and no session starts

#### Scenario: Subdomain
- **WHEN** the list is `example.com` and the verified address is `a@mail.example.com`
- **THEN** it is refused

#### Scenario: Unverified address
- **WHEN** the list is set and the provider did not verify the address
- **THEN** it is refused

#### Scenario: Tightened while pending
- **WHEN** a pending sign-up was made before the list excluded its address and the person presses Create
- **THEN** the response is 403 `sign_in_not_allowed` and the pending sign-up is gone

#### Scenario: Passkey
- **WHEN** a passkey that verifies belongs to a passkey-only account, or to an account whose addresses are not listed
- **THEN** the response is 403 `sign_in_not_allowed` and no session starts; with a listed, verified address on the account it signs in

#### Scenario: Refresh after the list tightened
- **WHEN** an account's refresh token is presented after the list stopped including it
- **THEN** the token endpoint answers `invalid_grant`, and after the list includes it again the same token still refreshes

#### Scenario: Adding a way in
- **WHEN** a signed-in person links a Zoho account whose verified address is not listed
- **THEN** it is not linked and the browser returns with `link_error=sign_in_not_allowed`

#### Scenario: No list
- **WHEN** the setting is unset, empty or only commas
- **THEN** behaviour is unchanged
