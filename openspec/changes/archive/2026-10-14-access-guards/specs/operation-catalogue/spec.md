## ADDED Requirements

### Requirement: A replay is authorized like the request it repeats
Before a stored response is replayed, the caller SHALL still hold the
operation's scopes (else 403), still belong to the workspace of a
`{workspace_id}` route (a client token still bound to it), and still see
every container the first run checked. Otherwise the answer SHALL be the
404 the operation itself would give, and the operation SHALL NOT run
again.

#### Scenario: Taken out of the inbox
- **WHEN** a member repeats, with the same key, a request whose response is stored, after being taken out of the inbox it acted in
- **THEN** the answer is 404 and no stored body is returned
