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
migrations/                <ts>_<module>__<desc>.sql, one Postgres schema per module;
                           <ts>_kernel__<desc>.sql for the kernel's `kernel` schema
openapi/openapi.json       generated, committed; the contract for every client
xtask/                     `cargo xtask arch | openapi [--check]`
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

Inside it, code is sliced **by use case**, not by layer:

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
passed, and `purge_workspace` removes a deleted workspace's blobs. The store
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
