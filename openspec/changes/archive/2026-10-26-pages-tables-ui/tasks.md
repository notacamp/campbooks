## 0. Design gate

- [x] 0.1 Prototype "Tables" approved, with part 1's sign-off ("go for it", 2026-10-26); this change is part 2 of the owner-requested split

## 1. Server

- [x] 1.1 `kind` on `PageSummary`/`PageView`; rows never listed; tests
- [x] 1.2 `tableView` block in the schema allowlist (ids only), links as embed; tests incl. an unreadable table

## 2. Client components

- [x] 2.1 `DataGrid` (virtualised, sticky header/first column, roving-focus keyboard navigation, resize) with stories and tests
- [x] 2.2 `CellEditor` for every type, option picker (create/rename/colour), row picker, person and Campbooks-link pickers by name
- [x] 2.3 `BoardView` with drag and a keyboard "Move to"; `RowPeek` (side panel, full screen on phones)
- [x] 2.4 `FilterBuilder`, `SortBuilder`, `PropertyTypeMenu`

## 3. Screens

- [x] 3.1 Table page: view tabs, one-row toolbar, column menu, summaries, `n r`, ⌘↵, undo toasts, optimistic cells with the version check
- [x] 3.2 Tree: table icon, "New table"; `/table` and `/link to table` blocks; "Something you can't open"
- [x] 3.3 Change-feed refresh on `table_changed`
- [x] 3.4 i18n en/pt-PT/es-ES/fr; `pnpm test:fit` 375/1280; coverage ≥ 80%; idle stories with a stateful mock + recording scenario `pages-tables.ts`

## 4. Finish

- [x] 4.1 Tracked events (patch 0019 if any) and Coverage rows: none needed; every screen action is one of part 1's operations, already tracked
- [x] 4.2 OpenAPI and client types regenerated
- [x] 4.3 Changelog entry `changelog/unreleased/added/365-pages-tables-ui.md`
- [x] 4.4 Gates green in CI
- [x] 4.5 Recording at 375 px and desktop
- [x] 4.6 `openspec archive pages-tables-ui --yes` before merge
