## ADDED Requirements

### Requirement: The identity module's changes are tracked
Every change the `identity` module makes SHALL be tracked as an event of the
tracking plan, in the transaction of the change, with a snapshot of the
records it touched read from the module's own rows after the change: sign-ins
that succeed or are refused (with the kind of refusal), accounts created,
sessions ended one by one or all at once, ways in and passkeys added, renamed
and removed, accounts suspended and unsuspended, apps registered, given a new
secret and removed, consents granted, denied and revoked, held mailbox grants
discarded and adopted, authorization codes exchanged for tokens, refresh
tokens reused and revoked. A change that changes nothing SHALL track nothing.
Events SHALL carry ids, enums, booleans, counts and times only: never an
address, a name, a provider subject, a token, a secret, a code, a credential
id, a user agent, a URL or an IP address. Steps taken before anyone holds a
token SHALL be tracked as actor `anonymous`, with the account by id in the
body. Every mutating operation, handler and schedule of the module SHALL
have a Coverage row.

#### Scenario: A person signs in with a provider
- **WHEN** a known provider identity comes back to the callback and its account may sign in
- **THEN** one `identity.sign_in_succeeded` event is written with actor `anonymous`, operation `identity.finish_sign_in` and the new session (its id, the account id, method `provider` and the provider), and no address

#### Scenario: A sign-in is refused
- **WHEN** the person cancels at the provider
- **THEN** one `identity.sign_in_failed` event is written with failure `cancelled_at_provider`, the provider, and no account and no address

#### Scenario: Signing out everywhere
- **WHEN** a person signs out everywhere with two live sessions
- **THEN** one `identity.sessions_ended` event is written with actor `user` and `sessions_ended` 2

#### Scenario: An app whose id is a URL
- **WHEN** an app known by its metadata document (its client id is a URL) is let in and exchanges its code, or a request runs as a `client` whose id Campbooks did not mint
- **THEN** its events name it by kind (`metadata_document`) and, as an actor, as `{"type": "client"}` with no id; the URL reaches no tracked event

#### Scenario: Refreshing tokens
- **WHEN** a client rotates its refresh token
- **THEN** nothing is tracked; presenting the rotated token again tracks `identity.refresh_reuse_detected`
