# Architecture

Campbooks is a **modular monolith**: one Rust API server composed from
domain modules, plus one TypeScript UI shipped to every platform through
Tauri. This page is the map; the *why* lives in `docs/adr/`.

```
                    ┌─────────────────────────────────────────────┐
  Web browser ──────┤ campbooks-server  (axum)                     │
  (cookie only)     │  /*       built web client + SPA fallback     │
                    │  /bff/*   session + OIDC code flow (ADR 0004) │
                    │  /api/v1/* resource server: JWT + scopes      │
  Tauri desktop ────┤  /openapi.json                                │
  Tauri iOS/Android │                                               │
  (token in OS      │  modules: system · conversations · schedule … │
   keychain, Rust)  └───────┬──────────────┬─────────────┬──────────┘
                            │              │             │
  External client ──OAuth──►│  Postgres    │  IdP        │  AI providers
  (client_credentials /     │  (+pgvector) │  (OIDC)     │  (OpenAI-compatible,
   auth code + PKCE)        │  schema per  │  ADR 0004   │   open-weight models,
                            │  module      │             │   ADR 0005)
```

## Repository layout

```
Cargo.toml                 workspace (kernel, ai, server, crates/modules/*, xtask)
architecture.toml          module dependency allowlist, read by `cargo xtask arch`
crates/
  kernel/                  campbooks-kernel — config, errors (problem+json), db pool,
                           auth extractors, ids, clock, durable event bus and job
                           queue (schema `kernel`), module declarations and the
                           action registry, Module contract.
                           Depends on NO internal crate.
  ai/                      campbooks-ai — role-based AI gateway: decide (typed decision
                           model, ADR 0009), embed/complete/vision (OpenAI-compatible
                           protocol). Depends on kernel only.
  server/                  campbooks-server — the binary. Loads config, runs migrations,
                           composes modules into one router, one job registry and
                           one declarations catalogue, generates an endpoint per
                           action, serves OpenAPI and, when CAMPBOOKS_WEB_DIR is set, the
                           built web client; runs as `all`, `api` or `worker`.
  modules/
    system/                campbooks-system — health, version. The reference slice:
                           copy its shape for every new module.
migrations/                <ts>_<module>__<desc>.sql, one Postgres schema per module;
                           <ts>_kernel__<desc>.sql for the kernel's `kernel` schema
openapi/openapi.json       generated, committed; the contract for every client
openapi/declarations.json  generated, committed; every record type, action and event
                           (clients build their shortcuts and command menu from it)
xtask/                     `cargo xtask arch | openapi [--check] | declarations [--check]`
clients/                   pnpm workspace
  app/                     the one UI (React + TypeScript + Vite)
    src/app/               shell: entry, router, providers, module manifest
    src/lib/               business-agnostic toolbox (api client, ui, i18n, auth bridge)
    src/modules/<domain>/  1:1 with crates/modules/<domain>
    src-tauri/             Tauri 2 shell (desktop + iOS + Android); its own Cargo
                           workspace so GUI system deps never enter server CI
deploy/                    docker compose for self-hosters (server, postgres, IdP, optional AI)
openspec/                  specs (baseline) and changes (in flight)
docs/adr/                  numbered architecture decision records
```

## Anatomy of a module

A module is a Rust crate that hands the server one value, its descriptor:

```rust
// crates/modules/<domain>/src/lib.rs
pub fn module() -> Module {
    Module {
        name: NAME,                          // = crate directory = Postgres schema
        scopes: scopes::ALL,                 // `<module>:<action>` OAuth scopes
        router: OpenApiRouter::new(),        // plain endpoints, if any
        background: Background::new(),       // jobs, event handlers, schedules
        declarations: Declarations::new()    // what the module has (ADR 0008)
            .record::<Conversation>()        // record types: `conversations.conversation`
            .action(features::archive::action())  // actions: `conversations.archive`
            .event::<ConversationArchived>(),     // events it publishes
    }
}
```

Inside it, code is sliced **by use case**, not by layer:

```
crates/modules/mail/src/
  lib.rs                 module() + the public event types other modules may subscribe to
  features/
    list_threads/        mod.rs (handler + request/response + query) and tests
    send_message/        mod.rs: `pub fn action()` (its declaration), the handler,
                         input/output types and tests calling it by name
  domain/                types shared by several features of THIS module only
```

A feature owns everything it needs; when two features in one module share
something, it moves to `domain/`. Nothing is shared across modules except
through the kernel (types, events) — `cargo xtask arch` enforces it.

### Declarations: record types, actions, events

A module declares what it has, and the platform generates the rest
(ADR 0008, `kernel::declare`):

- **Record types** (`RecordType`): `<module>.<record>`, the JSON Schema of
  the record's public shape, a label key and its **access container** —
  the workspace, the member, or another declared record (a mailbox, a
  calendar, a folder) whose sharing decides who sees it. Any record is
  pointed at with `RecordRef { type, id }`, which is how links, rules,
  search and attention reach across modules without knowing them.
- **Actions** (`Action` builder): `<module>.<verb>`, typed input and output,
  the scope a caller needs, whether it mutates, optional undo (with its
  window) and an optional default keyboard shortcut. The handler gets an
  `ActionContext` (state, principal, the call's transaction; the workspace
  joins it with the `workspaces` module) and does its writes and event
  publishing on `ctx.tx()`.
- **Events**: the `DomainEvent`s the module publishes, with their version
  and payload schema.

From these the server generates, for every action, `POST
/api/v1/actions/<module>.<verb>` — one namespace keyed by the action's name,
so a client can call any action it read from the catalogue, while each one is
still its own OpenAPI operation with its schemas and scope. The endpoint
enforces the scope like `Scoped<S>`, validates the input (422 problem+json),
honours `Idempotency-Key` on mutating actions (the response is stored in
`kernel.idempotency_keys` in the action's own transaction and replayed for a
repeat), and returns `{ result, undo: { token, expires_at } }` for undoable
ones; `POST /api/v1/actions/undo` redeems a token once, for the same caller
and workspace, before it expires. Expired keys and tokens are pruned hourly
(`kernel.prune_actions`).

`campbooks-server declarations` prints the catalogue (committed as
`openapi/declarations.json`, checked in CI): every record type, action
(scope, mutates, undo window, shortcut, label key, schemas, endpoint) and
event (version, payload schema), with the schemas under
`components.schemas`. The client's shortcut registry and command menu are
generated from it; MCP tools, assistant tools and automation steps will be
too (description and schemas for tools, `mutates` and undo to make
assistant changes proposals, events as automation triggers). Startup and
`cargo xtask arch` refuse misnamed, duplicate or inconsistent declarations
and any action that no test of its module calls by name.

### Data

Every module owns a Postgres schema of its own name and touches nothing else.
Other modules' records are referenced by ID only (no cross-schema foreign
keys or joins). The one schema that belongs to no module is `kernel`: the
event log and the job queue (ADR 0012), written only through the kernel's
API. `cargo xtask arch` checks that each migration touches only its owner's
schema. A read that needs data from two modules is composed in the
client, or by subscribing to the other module's events and keeping a local
projection.

### Endpoints and auth

The API is an OAuth 2.1 resource server (ADR 0004). A module declares its
scopes once (`<module>:<action>`, a `kernel::auth::ScopeName` type) and
lists them in `Module.scopes`. Every handler either requires one with the
`Scoped<S>` extractor and documents it with
`security(("bearer" = ["<scope>"]))`, or is explicitly public with
`security(())`. The extractor validates the bearer JWT against the IdP
configured by `CAMPBOOKS_AUTH__*` and yields the `Principal` (subject, kind,
client id, scopes, workspace). Bad tokens get 401, and missing scopes get 403.
Whether the principal may see a given record is the module's decision, and a
read it may not see answers 404. `campbooks-server scopes` prints the
catalogue the IdP mirrors. Module tests sign their own tokens with
`kernel::testing::TestIdp`.

### Cross-module communication

1. **Events (default).** A module declares its events as `DomainEvent` types
   (`NAME` = `<module>.<event>`, an explicit schema `VERSION`, the
   `workspace` they belong to) and publishes them **in the transaction that
   makes the change**: `state.events.publish(&mut tx, &event)`. That writes
   the event to the retained log `kernel.events` and one job per registered
   consumer to `kernel.jobs` (fan-out at publish time), so an event exists
   exactly when its change does, and every consumer runs it at least once,
   even across restarts. A consumer is an `EventHandler` type named
   `<module>.<handler>`, registered in the module's `background`
   (`Background::new().on_event::<H>()`). Its database work goes through
   `ctx.tx()`, which carries the workspace (`campbooks.workspace_id`) and
   commits together with the delivery being marked done. To rebuild a
   projection, `campbooks-server events replay <consumer> --since …`
   re-delivers logged events.
2. **Allowlisted dependency (exception).** If a module truly needs another's
   API, add `[[allow]] from = "x" to = "y" reason = "…"` to
   `architecture.toml`. Reviewed like an ADR.

### Background work

Anything that should not run inside a request — provider sync, the AI pass,
projections, maintenance — is a `Job` type (`NAME` = `<module>.<job>`, a
`QUEUE`, `MAX_ATTEMPTS`) registered with `.job::<J>()` and enqueued in a
transaction with `jobs::enqueue(&mut tx, &job, options)`. Options give it a
workspace, a delay, a priority, a `unique_key` (at most one unfinished job per
key) or an `ordering_key` (jobs sharing it run one at a time, in order).
Recurring work is `.schedule::<J>(every)`; each tick runs once however many
workers run. A handler returns `JobError::Retry` (backoff with jitter) or
`JobError::Fatal` (marked failed and kept for inspection). Workers claim jobs
fairly across workspaces, lease them while they run, and reclaim jobs whose
worker died. The design and its trade-offs are ADR 0012.

### Process roles

One binary, `campbooks-server`, runs as:

| Command | Runs | Use |
| --- | --- | --- |
| `all` (default) | API + job worker | self-hosting, development; what the image runs |
| `api` | HTTP API, BFF, web client | scale on requests |
| `worker` | event deliveries, jobs, schedules | scale on queue depth |

Any number of `api` and `worker` processes share one database. The worker is
tuned with `CAMPBOOKS_JOBS__*` (concurrency, per-workspace limit, poll
interval, lease, retention, shutdown grace, retry backoff). A third role,
`sync`, arrives with the connectors (ADR 0011).

## Clients

One React/TypeScript UI, three delivery targets (ADR 0003):

| Target | How it talks to the API | Where tokens live |
| --- | --- | --- |
| Web | `/bff/*` on the same origin, httpOnly cookie | server session store |
| Desktop (macOS/Windows/Linux) | Tauri command → Rust HTTP client | OS keychain |
| iOS / Android / tablets | Tauri command → Rust HTTP client | Keychain / Keystore |

`src/lib/api` hides the difference behind one typed client generated from
`openapi/openapi.json`, so modules never know which target they run on.

The web target is served by `campbooks-server` itself (ADR 0006): the release
image builds `clients/app` and sets `CAMPBOOKS_WEB_DIR`, so UI and API share
one origin and port. Until the BFF lands, `/bff/api/*` is an interim
passthrough to the same API routes with no session or credentials of its own
(`openspec/changes/serve-web-client/`).

## Domains and scaling

The module list, each module's language, how they talk and the order they
ship in are in [`domain-map.md`](domain-map.md). How the system scales — one
binary in `api`/`worker`/`sync` roles, connectors producing change streams,
projections from the event log, blobs in object storage, and the platform
seams every module declares — is in ADRs 0008, 0010, 0011 and 0012.
