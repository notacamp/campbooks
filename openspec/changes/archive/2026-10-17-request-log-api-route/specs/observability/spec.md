## MODIFIED Requirements

### Requirement: Request traces
The system SHALL record one server span per HTTP request named after the
method and route template, with the route template, method, status code,
request id and trace id, plus the workspace id when the route names a
workspace and the principal kind (`user` or `client`) when the caller is
authenticated. A W3C `traceparent` on the request SHALL make the span part of
that trace. A web BFF request that runs through an API route in-process SHALL
be named after that API route template, and its span SHALL carry
`campbooks.via=bff`.

#### Scenario: Incoming trace context
- **WHEN** a request arrives with `traceparent: 00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01`
- **THEN** its span and its log lines carry trace id `4bf92f3577b34da6a3ce929d0e0e4736`

#### Scenario: Through the web BFF
- **WHEN** the web client requests `GET /bff/api/v1/w/{id}/members`
- **THEN** its span is named `GET /api/v1/w/{workspace_id}/members` with `http.route=/api/v1/w/{workspace_id}/members` and `campbooks.via=bff`, and it is counted in that route's request duration series

#### Scenario: No API route
- **WHEN** the web client requests a path under `/bff/api/` that no API route matches
- **THEN** it is named and counted under `/bff/api/{*path}`

## ADDED Requirements

### Requirement: Request log
The system SHALL write one log line per HTTP request when it ends, with the
message `request finished`, the status and the duration in milliseconds, in
the request's span: method, route template, status code, request id, trace
id and, when known, workspace id, principal kind, the problem's code and
`campbooks.via`. The line SHALL NOT hold the request's path or query string.

#### Scenario: A web call
- **WHEN** the web client requests `GET /bff/api/v1/w/{id}/members?q=boiler`
- **THEN** one `request finished` line is written with status 200, a duration, `http.route=/api/v1/w/{workspace_id}/members` and `campbooks.via=bff`, and no field holds the path or the query string (`boiler` appears nowhere)
