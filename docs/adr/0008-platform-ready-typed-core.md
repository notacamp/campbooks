# 0008 — Typed core now, platform later, seams from day one

**Status:** Accepted · 2026-10-13 · declarations amended by [0013](0013-openapi-is-the-operation-catalogue.md)

## Context
Campbooks could be built two ways:

- **As an application** — mail, calendar, tasks, files and money as typed
  modules, each with its own tables and logic.
- **As a platform** — user-defined record types and fields, automation over
  any record, and a dashboard builder, with mail, calendar, tasks, files and
  money as built-in apps on those same primitives.

The platform is attractive: rules, saved views, an API, MCP tools and an
assistant all need the same thing — a uniform way to see every record type,
action and event — and without it each surface ends up re-declaring the
others' actions by hand. But building the generic layer first delays a
usable product, and the built-in domains are not generic: message sync,
two-way calendar sync and bank reconciliation need typed code, typed tables
and compile-time-checked queries.

## Decision
**Build the built-in domains as typed modules, but give every module the
seams a platform needs from the first module onwards. Build the platform
modules (`records`, `views`) last.**

The seams, all required of every module from M0:

1. **Declarations.** A module's `Module` descriptor declares its **record
   types** (name, fields, display, access container), its **actions**
   (name, input/output schema, required scope, whether it mutates, undo) and
   its **events** (name, payload schema). Schemas are JSON Schema generated
   from the Rust types.
2. **One action registry.** HTTP endpoints for actions, MCP tools, the
   assistant's tools, automation actions and the command palette are all
   generated from the declarations. Nothing is registered by hand twice.
3. **Record references.** Any record can be pointed at with a
   `RecordRef { type, id }`, so links between records, automation, search,
   attention and (later) views work across modules without knowing them.
4. **Durable, versioned events.** Domain events go through the outbox
   (issue #19) with a schema version, so rules and projections can rely on
   them.
5. **Rules from M2.** The `automation` module ships with the first real need
   (tidying the inbox) but is built over declared events and actions, so it
   works for every module, built-in or custom, from the start.

What is *not* built until M5: user-defined record types, custom fields on
built-in records (stored by `records` against a `RecordRef`, so built-in
tables never change shape), saved views beyond each module's own screens, and
dashboards.

## Consequences
- The first modules cost a little more (declarations, schemas), and in
  exchange the API, MCP, assistant and rules come largely for free.
- Built-in data keeps typed tables and sqlx-checked queries; no
  entity–attribute–value storage for the core.
- Custom records live in one module (`records`) with JSONB validated against
  the workspace's schema; they are slower to query than typed tables, which
  is acceptable for user-defined data.
- If the platform never ships, the seams still pay for themselves (one
  registry, one rule engine).

## Alternatives considered
- **Platform first** — everything as records from day one. Rejected: delays a
  usable inbox by months and makes the hardest domains (sync, reconciliation)
  harder.
- **Application only** — no declarations. Rejected: every surface (API, MCP,
  assistant, rules) re-declares actions by hand, and the platform becomes a
  rewrite later.
