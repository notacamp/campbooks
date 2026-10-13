# Design

## Context

Operations are `utoipa-axum` routes merged by the server into one
`OpenApiRouter`; `Scoped<S>` checks the bearer token and scope; events and
jobs are durable (ADR 0012) and registered per module in `Background`. ADR
0013 is the spec of record.

## Data ownership and events

- **Schema `kernel`**, table `kernel.idempotency`, migration
  `20261013000000_kernel__idempotency.sql`. Written only by the kernel's
  idempotency layer and its prune job. Primary key `(principal, key)`:
  `principal` is `user:<sub>` or `client:<client id>`; the row holds the
  `operation_id`, a SHA-256 of method, path, query and body, a state
  (`pending` / `completed`), the stored status, content type, `Location` and
  body, and `expires_at` (24 h).
- **Events:** none published or consumed. The mechanism to declare them is
  added and tested with test-only events.
- **Endpoints:** no new endpoint, so no new scope. `system.health` and
  `system.version` stay public (`security(())`).
- **AI:** none.

## Declaring an operation

`x-campbooks` is a JSON value built from the typed `kernel::Operation` inside
utoipa's own `extensions(...)` attribute (`json!(expr)` serializes the value
at document generation; no macro of ours). `Operation::query()` and
`Operation::command(risk)` start with automation and the assistant off.

## Checks

`cargo xtask arch` derives each operation's module from the server's
composition (each module's router rendered to OpenAPI on its own), not from
the path, so workspace routes (`/api/v1/w/{workspace_id}/…`) are owned
correctly. It then checks the rules in the proposal over the JSON document.

## Invoke by id

The server calls `operations::compose(api, state)`, which parses the
`Catalogue` (id → method, path template, metadata, scopes, body), wraps the
routes in the idempotency layer and stores them (without state) in
`AppState.operations`. `invoke` checks the metadata gates, builds the request
(path parameters by name from the input, `{workspace_id}` from the
invocation, the rest as JSON body or query), attaches the principal as a
kernel-private request extension and calls the router with the state attached
for that call. `Scoped<S>` uses that extension instead of the
`Authorization` header and still checks the scope; extensions cannot be set
from outside the process and the type is private to the kernel. The
operation's own extractors (scope, future `Workspace<S>` membership) run
unchanged, and failures come back as `InvokeError::Failed` with the
problem+json body.

## Idempotency

A `route_layer` (after routing, so `MatchedPath` names the operation; a
`/bff` prefix is stripped by looking from `/api/` on). GET/HEAD, requests
without the header, operations that do not mutate and unauthenticated
requests pass through. Otherwise the layer verifies the token once (handing
the principal to `Scoped`), buffers the body (10 MiB), and claims the key
with one `INSERT … ON CONFLICT DO UPDATE … WHERE expired OR abandoned`.
Conflicts read the row: other operation or hash → 422; pending → 409;
completed → replay with `Idempotency-Replayed: true`. After the handler, a
response below 500 (except 401, 403, 408, 409, 425, 429) and at most 1 MiB is
stored; otherwise the key is released. A pending key older than two minutes
(requests time out at 30 s) may be taken over by the identical request.

## Events catalogue

`Background::publishes::<E>()` records name, version, `WORKSPACE_SCOPED` and
`E`'s schema functions; `EventCatalogue::new(&registry)` renders events
sorted by name with inline payload schemas and referenced schemas under
`components.schemas` (OpenAPI 3.1 dialect). `DomainEvent` itself only gains
a defaulted constant, so existing implementors compile unchanged.

## Alternatives considered

- Module from the path prefix: breaks for workspace-scoped routes.
- Injecting a minted internal token for invoke: needs signing keys in the
  server and a second verifier; the in-process extension is simpler and
  unreachable from outside.
- `ToSchema` as a `DomainEvent` supertrait: breaks every implementor; the
  bound sits on `publishes` instead.
