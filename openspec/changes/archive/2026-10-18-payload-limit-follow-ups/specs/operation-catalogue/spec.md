## MODIFIED Requirements

### Requirement: Idempotent mutating operations
For every operation whose `x-campbooks.mutates` is true, a request with an
`Idempotency-Key` from an authenticated caller SHALL run at most once per key
and caller while the key is retained (24 hours): an identical retry SHALL
receive the stored response with `Idempotency-Replayed: true`; a different
request (operation, method, path, query or body) with the same key SHALL
receive 422; a retry while the first request is running SHALL receive 409.
A key whose first request ended without a stored answer SHALL never run
the operation again. That happens when its process died, when the request
was stopped at its time limit, or when the client or a proxy hung up, each
perhaps after committing its effects. Once the key has been pending for two
minutes, longer than any request runs, a retry SHALL receive 409
`idempotency_outcome_unknown` in the caller's language, until the key
expires. A request still running after the server's time limit (30 seconds)
SHALL be answered 503 `request_timed_out`, which does not release its key:
the caller checks the result, then retries with a new key. A server error
the operation itself answers still releases the key, so a retry runs it
again. If the operation committed before that error (or before an
acknowledgement was lost), only the operation's own guard (a draft's send
state, for mail) keeps the work from running twice. Therefore an operation
with an irreversible external effect (a send, a change at a provider, a
payment) SHALL have a domain-level guard, a state machine or a unique key on
the effect, and SHALL NOT rely on idempotency alone. A keyed request body
SHALL be at most 10 MiB, since the layer holds it to hash it. A larger
upload SHALL be stored in its own step without a key (blobs are content
addressed, so the same bytes are the same blob) and attached by reference in
a small keyed operation.
Server errors and authentication or rate-limit failures SHALL release the
key. GET and HEAD requests SHALL ignore the header, and no operation SHALL be
able to opt out. Expired keys SHALL be deleted by an hourly job. For an
operation declaring `x-campbooks.secret_once` (`identity.register_client`,
`identity.rotate_client_secret`, `workspaces.invite_member`,
`identity.start_link`, `connectors.start_connection`, `identity.token`), an
answer below 400 SHALL NOT be stored: the key SHALL be kept as used
(`withheld`: its status, and a success's `Location`, never a redirect's, no
body), and a replay SHALL answer 409 problem+json
`idempotency_secret_shown_once` with `Idempotency-Replayed: true`, in the
caller's language, after the same authorization as any replay, without
running the operation again or showing the secret.

#### Scenario: Identical retry
- **WHEN** a client sends the same POST twice with `Idempotency-Key: k-1`
- **THEN** both responses have the same status and body, the second carries `Idempotency-Replayed: true`, and the handler ran once

#### Scenario: Key reused for another body
- **WHEN** a client reuses `Idempotency-Key: k` with a different body
- **THEN** the response is a 422 problem+json

#### Scenario: Server error
- **WHEN** the first request with a key fails with 503
- **THEN** a retry with the same key runs the operation again

#### Scenario: A secret shown once
- **WHEN** an admin registers a confidential app with `Idempotency-Key: register-1` and sends the same request again with the same key
- **THEN** the first answer is 201 with the app's secret, the stored key holds no body and no secret, and the second answer is 409 `idempotency_secret_shown_once` marked replayed, without the secret

#### Scenario: The key of a secret reused
- **WHEN** the key of a withheld answer is reused for another request
- **THEN** the answer is 422, as for any reused key

#### Scenario: The token endpoint and a key
- **WHEN** a client calling `/oauth2/token` also sends a valid bearer token and an `Idempotency-Key`, then sends the same request again
- **THEN** the issued tokens are not stored, and the second request answers 409 `idempotency_secret_shown_once`

#### Scenario: A crash between the commit and the stored answer
- **WHEN** a request with `Idempotency-Key: k-9` commits its effects and its process stops before its answer is stored, and the same request is sent again right away, then after two minutes, then once more
- **THEN** the first retry receives 409 `idempotency_key_in_flight`, the later ones 409 `idempotency_outcome_unknown`, and the operation never runs a second time

#### Scenario: Two requests with one key at once
- **WHEN** two identical requests with the same key arrive while the first is still running
- **THEN** exactly one runs and the other receives 409 `idempotency_key_in_flight`, and a retry afterwards replays the one answer

#### Scenario: A request past its time limit
- **WHEN** a request is still running after the server's time limit
- **THEN** it is answered 503 `request_timed_out` in the caller's language, and a retry with the same key is not run again
