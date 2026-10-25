## ADDED Requirements

### Requirement: Idempotency request hashes are keyed
The hash the idempotency layer keeps to tell an identical retry from a
reused key (over the method, path, query and body of a keyed request) SHALL
be an HMAC-SHA256 under a key derived from the instance secret
(`CAMPBOOKS_SECRET_KEY`, HKDF-SHA256, label
`campbooks idempotency request v1`), never a plain digest of the request, so
that the database alone does not let anyone test a guess of a secret a body
carries (a mailbox password, a signup code, a token). Every process of an
install SHALL derive the same key; while a previous instance secret is set
(`CAMPBOOKS_SECRET_KEY_PREVIOUS`), a hash made under it SHALL still match.
The stored hash SHALL be compared in constant time. Without an instance
secret the server SHALL still start, keying its hashes with a random key of
its own and saying so at startup. A row stored with a hash made any other
way (before this requirement) SHALL answer a retry with 422
`idempotency_key_reused` and SHALL NOT run the operation again; such rows
SHALL NOT be deleted or rewritten before they expire.

#### Scenario: The stored hash is keyed
- **WHEN** a client connects an IMAP mailbox with `Idempotency-Key: c-1` and its password in the body
- **THEN** the stored request hash is not the SHA-256 of the request, and an identical retry replays

#### Scenario: Two installs, one request
- **WHEN** two installs with different instance secrets receive the same request with the same key
- **THEN** they store different request hashes

#### Scenario: A changed body
- **WHEN** a client reuses `Idempotency-Key: k` with a different body
- **THEN** the answer is 422, as before

#### Scenario: A row from before the change
- **WHEN** a key's row holds the plain SHA-256 of the request, unexpired, and the same request is sent again with that key
- **THEN** the answer is 422 `idempotency_key_reused`, the operation does not run, and the row is unchanged

#### Scenario: A rotation under way
- **WHEN** the instance secret is rotated with the previous one kept, and a request first sent before the rotation is retried with its key
- **THEN** the stored response is replayed
