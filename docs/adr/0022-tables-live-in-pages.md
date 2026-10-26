# 0022 — Tables live in pages, with property types records can adopt

**Status:** Proposed · 2026-10-26 · amends [0008](0008-platform-ready-typed-core.md)
· Tracking: notacamp/campbooks-cloud#365 · Change: `openspec/changes/pages-tables`

## Context
ADR 0008 keeps user-defined record types and fields for the platform modules
`records` and `views`, built last (M5). The owner made tables in Pages the
next priority (2026-10-25). These are Notion-style databases whose rows are
pages with typed properties, viewed as a table or a board, and they must be
able to hold a synced Notion database (cloud #377). Waiting for `records`
would hold tables back for months, and a row that is a page would sit in two
modules.

## Decision
1. **Tables are pages.** A table is a page of kind `table`, and its rows are
   pages of kind `row` under it. Access, trash, search, mentions, backlinks
   and Notion page sync all apply unchanged.
2. **The property types are a library.** `pages::properties` holds every
   type's config, value validation, comparison, SQL casting and conversion,
   and depends on nothing else in `pages`. When `records` is built, it adopts
   this library (moving it to the kernel if two modules need it) instead of
   defining a second type system.
3. **Typed storage stays typed where it matters.** Values are JSONB per cell,
   validated against the property type and cast by type in queries. That is
   ADR 0008's "custom records live in JSONB validated against the
   workspace's schema", applied to the first custom records.

## Consequences
- Tables ship without the platform modules, and their rows get everything
  pages already have.
- `records` later has two clients of one type system (tables and custom
  fields on built-in records), not two type systems.
- A table query is slower than a typed table, which ADR 0008 already accepts
  for user-defined data. Tables are capped at 20,000 rows until that is
  measured.

## Alternatives considered
- **Build `records` and `views` first.** Rejected: much larger, delays the
  owner's next priority, and splits a row from its page.
- **A Yjs document per table.** Rejected: filtering, sorting and summaries
  need SQL, and per-cell versions already keep concurrent edits apart.
