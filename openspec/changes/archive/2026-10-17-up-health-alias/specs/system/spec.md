## ADDED Requirements

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
