## ADDED Requirements

### Requirement: Streamed answers pass through the BFF
An API answer that streams (`text/event-stream`: the change feed, a
streamed reply) SHALL reach the browser through `/bff/api/<path>` as it is
written, never buffered whole by the BFF, for safe and unsafe methods
alike. An unsafe request SHALL still need the CSRF header and a same-origin
request before it reaches the API, and the request's time limit, including
any limit the operation raised for itself, SHALL apply to the in-process
request as to a direct one.

#### Scenario: A streamed reply through the BFF
- **WHEN** a signed-in browser POSTs a streamed reply to `/bff/api/v1/w/{workspace_id}/conversations/{conversation_id}/reply/draft/stream` with the CSRF header
- **THEN** the response is `text/event-stream` and its events arrive as the API writes them

#### Scenario: Without the CSRF header
- **WHEN** the same POST arrives without `X-Campbooks-CSRF`
- **THEN** the BFF refuses it before the API runs
