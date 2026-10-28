## 0. Design gate

- [x] 0.1 Prototype "Notion databases as tables" and part 1's sign-off ("LGTM", 2026-10-27); this is part 2 of the owner-requested split

## 1. Server

- [ ] 1.1 `connectors.list_notion_data_sources` (owner-only, flag, bounded, budget, fake Notion search over data sources); tests incl. another member's connection

## 2. Client

- [ ] 2.1 Import dialog: Databases group, import as tables, count toast incl. tables
- [ ] 2.2 Synced-table header badge, Stop syncing, Open in Notion, column-conflicts banner
- [ ] 2.3 Read-only Notion columns (lock + reason, view-only cells), "Campbooks only" and "Stays in Campbooks" marks, add-only options on synced tables
- [ ] 2.4 Cell conflict in grid and peek; resolve + Undo
- [ ] 2.5 "Remove from both places?" dialog sending `confirm_remote_delete=true` only on confirm; Undo restores in both places
- [ ] 2.6 i18n en/pt-PT/es-ES/fr; `pnpm test:fit` 375/1280; coverage ≥ 80%; idle stories with a stateful fake + recording scenario `pages-notion-databases.ts`

## 3. Finish

- [ ] 3.1 OpenAPI and client types regenerated (locally)
- [ ] 3.2 Changelog entry `changelog/unreleased/added/377-notion-databases-ui.md`
- [ ] 3.3 Gates green in CI
- [ ] 3.4 Recording at 375 px and desktop
- [ ] 3.5 `openspec archive notion-databases-ui --yes` before merge
