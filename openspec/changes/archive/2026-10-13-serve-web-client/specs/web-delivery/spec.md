## Purpose

How a single Campbooks server process delivers the built web client next to
the API on one origin, so one container image is a complete deployable app
(ADR 0006).

## ADDED Requirements

### Requirement: Optional web client directory
The server SHALL serve the built web client only when an operator configures a
web client directory (`CAMPBOOKS_WEB_DIR`). When it is not configured, the
server SHALL behave as an API-only server. When it is configured but contains
no `index.html`, the server SHALL refuse to start and name the setting in its
error.

#### Scenario: Directory not configured
- **WHEN** the web client directory is not configured and a client requests `/`
- **THEN** the response is 404 as `application/problem+json`

#### Scenario: Directory configured
- **WHEN** the web client directory is configured and a client requests `/`
- **THEN** the response is 200 with the directory's `index.html` as `text/html`

#### Scenario: Directory missing its entry point
- **WHEN** the server starts with a web client directory that has no `index.html`
- **THEN** it exits with a non-zero status and an error naming `CAMPBOOKS_WEB_DIR`

### Requirement: Single-page app fallback
When the web client is served, a `GET` or `HEAD` request for a path outside
the reserved API paths that matches no file, and whose `Accept` header
includes `text/html`, SHALL be answered with `index.html` so client-side
routes can be deep-linked and reloaded. Other unmatched requests SHALL get a
404 as `application/problem+json`, never `index.html`.

#### Scenario: Deep link
- **WHEN** a browser navigates to `/some/route` and no such file exists
- **THEN** the response is 200 with `index.html`

#### Scenario: Missing hashed asset
- **WHEN** a client requests `/assets/missing.js` and no such file exists
- **THEN** the response is 404 as `application/problem+json`, even if the request accepts HTML

#### Scenario: Non-navigation request for a missing file
- **WHEN** a client requests `/favicon.ico` without accepting `text/html` and no such file exists
- **THEN** the response is 404 as `application/problem+json`

### Requirement: Reserved API paths
Paths under `/api/`, under `/bff/`, and `/openapi.json` SHALL never be
answered by the web client. An unknown path there SHALL get a 404 as
`application/problem+json`, whether or not the web client is served.

#### Scenario: Unknown API path from a browser
- **WHEN** a browser navigates to `/api/v1/nope` while the web client is served
- **THEN** the response is 404 as `application/problem+json`

#### Scenario: Unknown BFF path
- **WHEN** a client requests `/bff/nope` while the web client is served
- **THEN** the response is 404 as `application/problem+json`

### Requirement: Caching
Content-hashed files under `/assets/` SHALL be served with
`Cache-Control: public, max-age=31536000, immutable`. `index.html`, the SPA
fallback and every other web client file SHALL be served with
`Cache-Control: no-cache`, so a new release is picked up on the next load.

#### Scenario: Hashed asset
- **WHEN** a client requests an existing file under `/assets/`
- **THEN** the response is 200 with `Cache-Control: public, max-age=31536000, immutable`

#### Scenario: Entry point
- **WHEN** a client requests `/` or a deep link that falls back to `index.html`
- **THEN** the response carries `Cache-Control: no-cache`

### Requirement: Security headers
Every response, API or web client, SHALL carry
`X-Content-Type-Options: nosniff`,
`Referrer-Policy: strict-origin-when-cross-origin` and
`X-Frame-Options: DENY`. Every HTML response from the web client SHALL carry a
Content-Security-Policy that allows scripts, styles and connections only from
the same origin, images from the same origin or `data:` URLs, no plugins, no
`<base>` element, no framing by any origin, and form submissions only to the
same origin.

#### Scenario: API response
- **WHEN** a client requests `/api/v1/system/version`
- **THEN** the response carries the three security headers

#### Scenario: HTML response
- **WHEN** a client requests `/`
- **THEN** the response carries the three security headers and a Content-Security-Policy with `default-src 'self'`, `script-src 'self'`, `object-src 'none'` and `frame-ancestors 'none'`

### Requirement: Interim BFF passthrough
Until the backend-for-frontend described in ADR 0004 exists, the server SHALL
answer `/bff/api/<path>` exactly as it answers `/api/<path>`. The passthrough
SHALL NOT create sessions or attach credentials, so it never grants access the
caller would not have on `/api/<path>` directly.

#### Scenario: Version through the passthrough
- **WHEN** a client requests `/bff/api/v1/system/version`
- **THEN** the response has the same status and body as `/api/v1/system/version`

#### Scenario: Unknown path through the passthrough
- **WHEN** a client requests `/bff/api/v1/nope`
- **THEN** the response is 404 as `application/problem+json`
