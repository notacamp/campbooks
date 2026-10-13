# Tasks

## 1. Declarations and checks

- [x] 1.1 `kernel::Operation`, `Risk`, `EXTENSION`; `system.health` / `system.version` with `x-campbooks`; regenerate `openapi/openapi.json` and client types
- [x] 1.2 `cargo xtask arch`: operationId presence, module prefix (from the composition), uniqueness, complete `x-campbooks`, `mutates` vs method, own records, undo targets; xtask unit tests
- [x] 1.3 `RecordRef { type, id }` with serde and `ToSchema`

## 2. Events catalogue

- [x] 2.1 `DomainEvent::WORKSPACE_SCOPED`, `Background::publishes`, registry checks, `ConsumerInfo.event_version`; kernel tests
- [x] 2.2 `EventCatalogue`, `campbooks-server events catalogue`, `cargo xtask events [--check]`, `openapi/events.json`, CI step
- [x] 2.3 `cargo xtask arch`: consumed events are published at the same version

## 3. Invoke and idempotency

- [x] 3.1 `Catalogue`, `Operations`, `compose`, `invoke`; `Scoped` accepts the in-process principal; tests: disallowed caller refused, missing scope 403, unconfirmed high-risk and keyless writes refused, query and path input, replay through invoke
- [x] 3.2 Migration `kernel__idempotency`, middleware, `kernel.prune_idempotency` hourly; DB tests: replay, 422, 409 and takeover, release on 5xx, reads ignore the header, bad keys 400, keys per caller, anonymous pass-through, expiry and pruning
- [x] 3.3 Server composes through `compose`; server test that invoke is gated by metadata

## 4. Wrap-up

- [x] 4.1 `docs/architecture.md`, `AGENTS.md`, CHANGELOG
- [x] 4.2 OpenAPI regenerated, client types regenerated, `.sqlx` regenerated, gates green (no UI change)
