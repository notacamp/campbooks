# 0013 — The OpenAPI document is the operation catalogue

**Status:** Accepted · 2026-10-13 · amends [0008](0008-platform-ready-typed-core.md)

## Context
ADR 0008 asks every module to declare its operations once, so the API, client
types, the command menu and shortcuts, the assistant's tools, automation and
MCP stay in agreement. A first implementation built a second, hand-made
registry beside the API (generic `POST /actions/{name}` endpoints, undo
tokens, shortcuts declared in the backend). That reproduces the shape that
caused drift before: two descriptions of the same operations to keep in sync,
and user-interface concerns living in the server.

## Decision
The **OpenAPI document generated from the code** (utoipa) is the single
catalogue of operations. Nothing else describes them.

- **Identity.** Every operation has a stable `operationId` named
  `<module>.<verb>[_<noun>]` (e.g. `conversations.archive`). It is a wire
  contract, like scope names: never renamed.
- **Metadata** lives in one vendor extension, `x-campbooks`, on each
  operation: `mutates` (bool), `undo` (the `operationId` of its inverse, when
  one exists), `automation` and `assistant` (whether rules and the assistant may
  call it), `risk` (`low|high`; high-risk calls always need confirmation), and
  `records` (the record types it touches). Scopes stay in OpenAPI `security`.
- **Events** (triggers for rules, inputs for projections) are listed in a
  generated `openapi/events.json`: name, version, workspace-scoped, payload
  JSON Schema, from the `DomainEvent` types (ADR 0012).
- **Invocation by id.** The kernel can run any operation in-process by
  `operationId` on behalf of a principal (the same router, the same scope and
  membership checks), which is how automation, the assistant and MCP call
  built-in operations.
- **Idempotency** is one middleware for every mutating operation
  (`Idempotency-Key`), not a per-operation feature.
- **Undo** is an operation, not a token: an operation names its inverse in
  `x-campbooks.undo`; the activity log (attention module) offers it.
- **Shortcuts and labels are the client's.** The client's shortcut registry is
  keyed by `operationId`; the backend never declares keys.
- **`RecordRef { type, id }`** stays in the kernel for cross-module references.
- **User-defined custom actions** (call a webhook, post to Slack, create a
  Notion row) are data owned by the automation module, with their own
  connections and credentials; they are not code-level declarations.
- `cargo xtask arch` enforces: every operation has an `operationId` with its
  module's prefix, an `x-campbooks` block, explicit security, and every
  `undo` target exists.

## Consequences
- One source of truth, produced by the code and checked in CI; standard
  tooling (client generators, OpenAPI-to-MCP) works on it.
- Module authors write ordinary handlers; the extra cost is an `operationId`
  and a small extension block per operation.
- Automation steps for built-in behaviour exist as soon as the operation does.

## Alternatives considered
- **A separate action registry** (generic action endpoints, undo tokens,
  backend-declared shortcuts): a second catalogue to keep in sync with OpenAPI,
  user-interface concerns in the server, and an in-house framework to
  maintain. Rejected; this was the paused first attempt at #50.
- **Commands on the event bus** (every write is an asynchronous command):
  natural idempotency and audit, but asynchronous responses and error handling
  everywhere, and heavy for a single-process self-host. Rejected.
