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
  (token in OS      │  modules: system · conversations · schedule …│
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
                           auth extractors, ids, clock, event bus, Module trait.
                           Depends on NO internal crate.
  ai/                      campbooks-ai — role-based AI gateway: decide (typed decision
                           model, ADR 0009), embed/complete/vision (OpenAI-compatible
                           protocol). Depends on kernel only.
  server/                  campbooks-server — the binary. Loads config, runs migrations,
                           composes modules into one router, serves OpenAPI and,
                           when CAMPBOOKS_WEB_DIR is set, the built web client.
  modules/
    system/                campbooks-system — health, version. The reference slice:
                           copy its shape for every new module.
migrations/                <ts>_<module>__<desc>.sql, one Postgres schema per module
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
pub fn module() -> campbooks_kernel::Module { … }   // name, router, OpenAPI, event handlers
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
keys or joins). A read that needs data from two modules is composed in the
client, or by subscribing to the other module's events and keeping a local
projection.

### Cross-module communication

1. **Events (default).** A module publishes typed domain events on the kernel
   event bus (in-process today; durable via the job queue later). Others
   subscribe.
2. **Allowlisted dependency (exception).** If a module truly needs another's
   API, add `[[allow]] from = "x" to = "y" reason = "…"` to
   `architecture.toml`. Reviewed like an ADR.

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
seams every module declares — is in ADRs 0008, 0010 and 0011.
