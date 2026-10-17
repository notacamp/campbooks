# system Specification

## Purpose
Operational endpoints every deployment needs: liveness/readiness for
orchestrators and the running version for support and clients. This is also
the reference module whose shape every other module copies.

## Requirements

### Requirement: Health check
The system SHALL expose `GET /api/v1/system/health`, without authentication,
reporting whether the API process and its database are usable.

#### Scenario: Database reachable
- **WHEN** a client requests the health endpoint and the database answers a trivial query
- **THEN** the response is 200 with `status` = `ok` and `database` = `ok`

#### Scenario: Database unreachable
- **WHEN** a client requests the health endpoint and the database cannot be queried
- **THEN** the response is 503 as `application/problem+json`, without leaking connection details

### Requirement: Version
The system SHALL expose `GET /api/v1/system/version`, without authentication,
returning the semantic version of the running build and, when known, the git
commit it was built from.

#### Scenario: Version requested
- **WHEN** a client requests the version endpoint
- **THEN** the response is 200 with `version` (semver string) and `commit` (string or null)

### Requirement: Container health probe
The server binary SHALL provide a `healthcheck` subcommand that exits 0 when
the local health endpoint reports `ok` and non-zero otherwise, so container
images without a shell can declare a health check.

#### Scenario: Healthy server
- **WHEN** `campbooks-server healthcheck` runs inside a container whose server is healthy
- **THEN** it exits with status 0

### Requirement: Probe alias
The server SHALL answer `GET /up`, without authentication, with the same
status and body as the health check, uncached, and outside the API's OpenAPI
document, so that load balancers and uptime monitors that probe `/up` work.
The web client SHALL never answer it.

#### Scenario: Database reachable
- **WHEN** `GET /up` is requested and the database answers
- **THEN** the response is 200 with `{"status":"ok","database":"ok"}` and `Cache-Control: no-store`

#### Scenario: Database unreachable
- **WHEN** `GET /up` is requested and the database does not answer
- **THEN** the response is 503 `application/problem+json` with the same `code` as the health check, in the caller's language

#### Scenario: Browser navigation
- **WHEN** a browser navigates to `/up` (`Accept: text/html`) while the web client is served
- **THEN** it gets the health answer, not the web client

#### Scenario: Not an API operation
- **WHEN** the OpenAPI document is read
- **THEN** it has no `/up` path
