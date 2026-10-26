## 0. Design gate

- [x] 0.1 Owner decisions (Table + Board; computed columns later, read-only from Notion); proposal, design with alternatives, spec deltas; prototype board "Tables"
- [x] 0.2 Owner sign-off on the prototype and design ("go for it", 2026-10-26)
- [x] 0.3 ADR 0022 "Tables live in pages; their property types are a library records can adopt" (amends ADR 0008)

## 1. Model

- [ ] 1.1 Migration (version from 20261110010000, tracking patch 0018, per the merge coordinator): `pages.pages.kind`, `table_properties`, `row_values`, `table_views`, unique-id sequence; forced RLS; flag `CAMPBOOKS_PAGES__TABLES__ENABLED` with the flag-off production-values test
- [ ] 1.2 `pages::properties` library: every type's config + value validation, option ids, comparison, SQL casts, conversions on type change; property-based tests per type
- [ ] 1.3 Access through the page chain (table for schema/views, row for values); reader 403 tests

## 2. Operations

- [ ] 2.1 `create_table`, `add/update/remove_property`, `set_options` with undo
- [ ] 2.2 `add_row`, `set_cells` with per-cell versions (stale write answers the replaced value), paired relations in one transaction, Campbooks link refs only
- [ ] 2.3 `query_table` (filter tree, sorts, cursor, summaries) in SQL; performance test at 20,000 rows
- [ ] 2.4 Views CRUD (table, board; reserved types refused); board move = set value with undo
- [ ] 2.5 Events (`page_changed` kinds, `table_changed`), search over rows, workspace purge

## 3. Client

- [x] 3.1 None in this part: the table and board screens, row peek, column menu, inline blocks and recording are part 2 (`pages-tables-ui`)

## 4. Finish

- [ ] 4.1 Tracked events and Coverage rows
- [ ] 4.2 OpenAPI and client types regenerated (CI artifacts)
- [ ] 4.3 Changelog entry `changelog/unreleased/added/365-pages-tables.md`
- [ ] 4.4 Gates green in CI
- [ ] 4.5 Recording: a terminal demo of the API (vhs, demo data), since this part has no UI
- [ ] 4.6 `openspec archive pages-tables --yes` before merge
