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
                           queue (schema `kernel`), blob storage, Module trait.
                           Depends on NO internal crate.
  ai/                      campbooks-ai — role-based AI gateway: decide (typed decision
                           model, ADR 0009), embed/complete/vision (OpenAI-compatible
                           protocol). Depends on kernel only.
  server/                  campbooks-server — the binary. Loads config, runs migrations,
                           composes modules into one router and one job registry,
                           serves OpenAPI and, when CAMPBOOKS_WEB_DIR is set, the
                           built web client; runs as `all`, `api` or `worker`.
  modules/
    system/                campbooks-system — health, version. The reference slice:
                           copy its shape for every new module.
    workspaces/            campbooks-workspaces — tenancy: users, workspaces, members
                           and roles, invitations, signup policy, security log;
                           implements the kernel's `Membership` port.
migrations/                <ts>_<module>__<desc>.sql, one Postgres schema per module;
                           <ts>_kernel__<desc>.sql for the kernel's `kernel` schema
openapi/openapi.json       generated, committed; the operation catalogue and the
                           contract for every client (ADR 0013)
openapi/events.json        generated, committed; the events catalogue
xtask/                     `cargo xtask arch | openapi [--check] | events [--check] |
                           new-module | new-feature`
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

A module is a Rust crate that hands the server one value:

```rust
// crates/modules/<domain>/src/lib.rs
pub fn module() -> campbooks_kernel::Module { … }   // name, scopes, router, background work
```

`cargo xtask new-module <name>` scaffolds one (crate, schema migration,
OpenSpec change, client module) and `cargo xtask new-feature <module>
<feature>` adds a slice. Inside it, code is sliced **by use case**, not by
layer:

```
crates/modules/mail/src/
  lib.rs                 module() + the public event types other modules may subscribe to
  features/
    list_threads/        mod.rs (handler + request/response + query) and tests
    send_message/
  domain/                types shared by several features of THIS module only
```

A feature owns everything it needs; when two features in one module share
something, it moves to `domain/`. Nothing is shared across modules except
through the kernel (types, events) — `cargo xtask arch` enforces it.

### Data

Every module owns a Postgres schema of its own name and touches nothing else.
Other modules' records are referenced by ID only (no cross-schema foreign
keys or joins). The one schema that belongs to no module is `kernel`: the
event log and the job queue (ADR 0012), written only through the kernel's
API. `cargo xtask arch` checks that each migration touches only its owner's
schema. A read that needs data from two modules is composed in the
client, or by subscribing to the other module's events and keeping a local
projection.

**Tenancy in depth (ADR 0011).** Every workspace-owned table has a
`workspace_id` column, and queries filter on it. Behind that, Postgres
row-level security (enabled and forced) admits only the rows of the
workspace set for the transaction in `campbooks.workspace_id`:
`state.db.begin_for(workspace)` (`kernel::db::BeginFor`) for requests, and
`ctx.tx()` for jobs and event handlers. Superusers skip policies, so the
server never queries as the user it logs in with: migrations run first on a
connection of their own, then every pooled connection switches to the
runtime role `campbooks_app` (`CAMPBOOKS_DATABASE_ROLE`), which the kernel
migration creates. Each migration grants that role what its own tables need
and declares the policies of its workspace-owned tables (with
`nullif(current_setting('campbooks.workspace_id', true), '')::uuid`); a
server test fails if a table with a `workspace_id` column has no forced
policy, or if the runtime role cannot use a table. Module tests run under
the same role with `kernel::testing::app_role_pool`.

### Files (blobs)

Message bodies (raw MIME), attachments and files are not stored in
Postgres but in object storage (ADR 0011), through `state.blobs`
(`kernel::blobs::BlobStore`): local disk by default, any S3-compatible
bucket otherwise (`CAMPBOOKS_BLOBS__*`). Blobs are content-addressed per
workspace (`<workspace>/<aa>/<sha256>`), so the same bytes are stored once.
A module `put`s the bytes (streamed and hashed; it gets a `BlobRef` of
hash, size and content type to keep on its record), then `attach`es the blob
to its record in the transaction that writes the record, and `detach`es it in
the transaction that removes it. The kernel's hourly `kernel.sweep_blobs`
deletes blobs nothing holds once `CAMPBOOKS_BLOBS__SWEEP_GRACE_HOURS` have
passed, and `purge_workspace` removes a deleted workspace's blobs (the
`workspaces.purge_workspace` job calls it when the grace period ends). The store
never serves blobs by URL: a module's download endpoint checks access, then
streams `get`.

### Endpoints and auth

The API is an OAuth 2.1 resource server (ADR 0004). A module declares its
scopes once (`<module>:<action>`, a `kernel::auth::ScopeName` type) and
lists them in `Module.scopes`. Every handler either requires one with the
`Scoped<S>` extractor and documents it with
`security(("bearer" = ["<scope>"]))`, or is explicitly public with
`security(())`. The extractor validates the bearer JWT against the IdP
configured by `CAMPBOOKS_AUTH__*` and yields the `Principal` (subject, kind,
client id, scopes, workspace, and a user's email and name). Bad tokens get
401, and missing scopes get 403. Whether the principal may see a given
record is the module's decision, and a read it may not see answers 404.

Workspace-owned resources live under `/api/v1/w/{workspace_id}/<module>/…`
(the workspace itself is `/api/v1/w/{workspace_id}`). Their handlers take
`kernel::tenancy::Workspace<S>` instead of `Scoped<S>`: after the token and
scope checks it reads the workspace from the path, requires a client
token's `workspace_id` claim to name it, and asks the `Membership` port —
a kernel trait the `workspaces` module implements and the server registers
in `AppState` — whether a user is a member, and with which role. Anything
else is 404. Handlers then open `state.db.begin_for(workspace.id)`. The path,
not a header, carries the workspace so URLs name it, generated clients must
pass it, and two tabs can work in two workspaces. Every operation's OpenAPI
`operation_id` is its catalogue name, `<module>.<verb>[_<noun>]` (ADR 0013). `campbooks-server scopes` prints the
catalogue the IdP mirrors. Module tests sign their own tokens with
`kernel::testing::TestIdp`.

### Operations: the catalogue (ADR 0013)

The generated OpenAPI document is the one list of what Campbooks can do.
There is no other registry: the command menu, the assistant's tools,
automation steps and MCP all read it. An operation is an ordinary handler
whose `#[utoipa::path]` carries three things besides its route and
responses:

```rust
#[utoipa::path(
    post,
    path = "/api/v1/conversations/threads/{id}/archive",
    operation_id = "conversations.archive",            // <module>.<verb>[_<noun>], never renamed
    security(("bearer" = ["conversations:write"])),    // or security(()) when public
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("conversations.unarchive")               // the inverse, itself an operation
        .automation()                                  // rules may call it
        .assistant()                                   // the assistant may call it
        .records(["conversations.thread"])))),         // the record types it touches
    responses(…),
)]
```

`Operation::query()` starts a read (no mutation, low risk) and
`Operation::command(risk)` a write; both expose nothing to automation or the
assistant until opted in. High-risk operations always need the user's
confirmation. Undo is an operation, named in `undo`, which the activity log
offers; shortcuts and labels belong to the client, keyed by `operationId`.
`cargo xtask arch` fails unless every operation has an `operationId`
prefixed by the module whose router serves it (and unique), a complete
`x-campbooks` block whose `records` are the module's own, explicit security,
`mutates` matching the method (POST/PUT/PATCH/DELETE vs GET/HEAD), and an
`undo` naming an existing mutating operation.

**Calling an operation from inside.** Automation, the assistant and MCP run
built-in behaviour with `kernel::operations::invoke(&state, Invocation)`:
the `operationId`, the input (path parameters by name; the rest is the JSON
body, or the query for operations without one), the principal it acts for,
the caller kind and, for `{workspace_id}` routes, the workspace. The kernel
refuses operations that do not opt the caller in, unconfirmed high-risk
calls, and mutating calls without an idempotency key; then it dispatches a
request through the very router HTTP traffic reaches, with the principal
attached in-process (nothing outside the process can set it), so the
handler's scope, membership and record checks apply unchanged. The server
builds that router with `kernel::operations::compose`.

**Idempotency.** One layer in front of every module route honours
`Idempotency-Key` on mutating operations (decided by the catalogue; no
per-operation opt-out): the first request's response is stored in
`kernel.idempotency` for 24 hours and replayed for an identical retry
(`Idempotency-Replayed: true`); a different request with the same key is
422, one still running is 409. Keys belong to the authenticated caller;
server errors and auth failures release the key. GET and HEAD ignore the
header. The kernel's `kernel.prune_idempotency` schedule deletes expired
keys hourly.

**Record references.** A module that points at another module's record
stores a `kernel::RecordRef { type, id }`, with `type` =
`<module>.<record>`, never a foreign key.

### Cross-module communication

1. **Events (default).** A module declares its events as `DomainEvent` types
   (`NAME` = `<module>.<event>`, an explicit schema `VERSION`, the
   `workspace` they belong to, and `WORKSPACE_SCOPED` when they are
   instance-wide) and publishes them **in the transaction that
   makes the change**: `state.events.publish(&mut tx, &event)`. That writes
   the event to the retained log `kernel.events` and one job per registered
   consumer to `kernel.jobs` (fan-out at publish time), so an event exists
   exactly when its change does, and every consumer runs it at least once,
   even across restarts. A consumer is an `EventHandler` type named
   `<module>.<handler>`, registered in the module's `background`
   (`Background::new().on_event::<H>()`). Its database work goes through
   `ctx.tx()`, which carries the workspace (`campbooks.workspace_id`) and
   commits together with the delivery being marked done. The publisher
   declares each event type with `Background::new().publishes::<E>()` (`E`
   also derives `utoipa::ToSchema`); that lists it in the generated
   `openapi/events.json` (name, version, workspace-scoped, payload JSON
   Schema), which `cargo xtask events --check` keeps current. `cargo xtask
   arch` fails when a handler consumes an event no module publishes, or at
   another version. To rebuild a
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

### AI calls

A module asks `campbooks-ai` for a **role**, never a vendor (ADR 0005).
`embed`, `complete` and `vision` use the OpenAI-compatible protocol. `decide`
is typed (ADR 0009): `Gateway::decide(ctx, state, questions)` takes a compact
state built by the module and `choice` / `score` / `noul` questions, and
returns one answer per question with a probability per option and a
confidence. It is served by Laya over `laya-serve` (`protocol = laya`, the
default) or by an instruct model through a JSON-schema adapter
(`protocol = openai`, probabilities from logprobs when available). Questions
are checked before any call (at most 20 options: split larger sets into a
coarse choice and a fine one). Probabilities pass through a per (question
type, option count) temperature, from config today and refitted from user
corrections later. A module acts on an answer only through
`answer.accept_if(threshold)` and asks the user below it.

## Clients

One React/TypeScript UI, three delivery targets (ADR 0003):

| Target | How it talks to the API | Where tokens live |
| --- | --- | --- |
| Web | `/bff/*` on the same origin, httpOnly cookie | server session store |
| Desktop (macOS/Windows/Linux) | Tauri command → Rust HTTP client | OS keychain |
| iOS / Android / tablets | Tauri command → Rust HTTP client | Keychain / Keystore |

`src/lib/api` hides the difference behind one typed client generated from
`openapi/openapi.json`, so modules never know which target they run on.

Screens are composed from the design system in `src/lib/ui` (three-layer
tokens, themes, components) and `src/lib/shortcuts` (one keyboard shortcut
registry); modules use them through `lib/ui/index.ts` and style nothing
themselves. See [`design-system.md`](design-system.md).

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
