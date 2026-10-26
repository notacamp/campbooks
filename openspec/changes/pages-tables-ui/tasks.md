## 0. Design gate

- [x] 0.1 Prototype "Tables" approved, with part 1's sign-off ("go for it", 2026-10-26); this change is part 2 of the owner-requested split

## 1. Server

- [ ] 1.1 `kind` on `PageSummary`/`PageView`; rows never listed; tests
- [ ] 1.2 `tableView` block in the schema allowlist (ids only), links as embed; tests incl. an unreadable table

## 2. Client components

- [ ] 2.1 `DataGrid` (virtualised, sticky header/first column, roving-focus keyboard navigation, resize) with stories and tests
- [ ] 2.2 `CellEditor` for every type, option picker (create/rename/colour), row picker, person and Campbooks-link pickers by name
- [ ] 2.3 `BoardView` with drag and a keyboard "Move to"; `RowPeek` (side panel, full screen on phones)
- [ ] 2.4 `FilterBuilder`, `SortBuilder`, `PropertyTypeMenu`

## 3. Screens

- [ ] 3.1 Table page: view tabs, one-row toolbar, column menu, summaries, `n r`, ⌘↵, undo toasts, optimistic cells with the version check
- [ ] 3.2 Tree: table icon, "New table"; `/table` and `/link to table` blocks; "Something you can't open"
- [ ] 3.3 Change-feed refresh on `table_changed`
- [ ] 3.4 i18n en/pt-PT/es-ES/fr; `pnpm test:fit` 375/1280; coverage ≥ 80%; idle stories with a stateful mock + recording scenario `pages-tables.ts`

## 4. Finish

- [ ] 4.1 Tracked events (patch 0019 if any) and Coverage rows
- [ ] 4.2 OpenAPI and client types regenerated
- [ ] 4.3 Changelog entry `changelog/unreleased/added/365-pages-tables-ui.md`
- [ ] 4.4 Gates green in CI
- [ ] 4.5 Recording at 375 px and desktop
- [ ] 4.6 `openspec archive pages-tables-ui --yes` before merge
