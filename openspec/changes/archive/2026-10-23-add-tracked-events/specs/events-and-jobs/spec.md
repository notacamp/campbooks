## ADDED Requirements

### Requirement: Jobs remember who caused them
When a job is enqueued, or a delivery fanned out, inside a tracking context,
the job row SHALL store that context's actor, on-behalf-of and correlation
(ids and enums only) as its origin, and the worker SHALL restore it as the
job's tracking context with actor `system` (the job's kind) acting on behalf
of the origin's actor.

#### Scenario: A delivery of an event published in a request
- **WHEN** Sam's request publishes a domain event and a consumer's delivery runs
- **THEN** events that delivery tracks name Sam as `on_behalf_of` and carry the request's correlation id
