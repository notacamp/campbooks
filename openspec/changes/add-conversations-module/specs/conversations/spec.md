## Purpose

TODO(new-module): what the `conversations` capability covers, in two sentences.

## ADDED Requirements

### Requirement: Conversations scopes
The `conversations` module SHALL declare the scopes `conversations:read` and
`conversations:write`, and every endpoint it adds SHALL require one of them.

#### Scenario: Anonymous caller
- **WHEN** a client calls a `conversations` endpoint without an access token
- **THEN** the response is 401 as `application/problem+json`

#### Scenario: Token without the scope
- **WHEN** a client calls `GET /api/v1/conversations/overview` with a token that lacks `conversations:read`
- **THEN** the response is 403 as `application/problem+json` naming the scope

### Requirement: TODO(new-module) first behaviour
TODO(new-module): replace this requirement with the module's first real
behaviour, stated with the normative keyword, and one scenario per case.
Until then this change fails `openspec validate --strict`, so it cannot merge
unfinished.

#### Scenario: TODO(new-module)
- **WHEN** TODO(new-module)
- **THEN** TODO(new-module)
