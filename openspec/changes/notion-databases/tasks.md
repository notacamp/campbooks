## 0. Design gate

- [x] 0.1 Owner decisions (import then two-way, picked only, per member; **columns both ways**, 2026-10-27); proposal, design, deltas; prototype board "Notion databases as tables"
- [x] 0.2 Owner sign-off on this proposal ("LGTM", 2026-10-27)
- [ ] 0.3 Owner: add `data_source.content_updated` and `data_source.schema_updated` to the Notion webhook subscription (polling covers until then)

## 1. Connectors

- [ ] 1.1 Flag `CAMPBOOKS_CONNECTORS__NOTION__DATABASES__ENABLED` + flag-off production-values test; capability/stream kind `notion.data_source`
- [ ] 1.2 Client calls (retrieve/query data source, page properties pagination, update schema, create/update/trash pages) on the fake Notion; budgets
- [ ] 1.3 Stream: schema hash, incremental entries, normalised hashed values, entry bodies linked as page streams; hook events mark due
- [ ] 1.4 Outbound kinds with MAC + base checks; column removal only with confirmation; never delete a database; tests

## 2. Pages

- [ ] 2.1 Migration (from 20261112010000): `external_tables`, `external_properties`, `external_cells`, `table_conflicts`, forced RLS
- [ ] 2.2 Import of data sources into a private linked table (properties, rows, default view)
- [ ] 2.3 Snapshot apply: schema three-way, cells three-way, read-only kinds, keep-on-remove-vs-edit; one transaction; tests for every case
- [ ] 2.4 Outbound diff (debounced); `confirm_remote_delete` on `remove_property` (400 `confirmation_required`); `restore_property` recreates in Notion with values
- [ ] 2.5 `pages.resolve_table_conflict` with undo; stopping rules; no values/names in events or logs

## 3. Finish

- [ ] 3.1 Tracked events (patch 0019) and Coverage rows
- [ ] 3.2 OpenAPI/events/client types regenerated (locally)
- [ ] 3.3 Changelog entry `changelog/unreleased/added/377-notion-databases.md`
- [ ] 3.4 Gates green in CI
- [ ] 3.5 Recording: vhs clip of the CI tests (server-only part)
- [ ] 3.6 `openspec archive notion-databases --yes` before merge
