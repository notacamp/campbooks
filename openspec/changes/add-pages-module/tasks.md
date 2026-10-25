## 0. Design gate

- [x] 0.1 Proposal, design with alternatives, spec deltas, visual prototype
- [x] 0.2 Owner sign-off on the prototype (2026-10-25)

## 1. Module and schema

- [x] 1.1 Crate `crates/modules/pages` following `tasks`' layout (lib.rs `module()`, domain/, features/, events.rs, scopes.rs, config.rs with `CAMPBOOKS_PAGES__ENABLED`, testkit.rs, i18n in four locales); registered in root `Cargo.toml`, `crates/server/Cargo.toml` and `modules_with`; `yrs` pinned
- [x] 1.2 Migration `<ts>_pages__init.sql`: `pages`, `updates`, `snapshots`, `grants`, `links` with forced RLS, `workspace_isolation`, grants and indexes as in design.md
- [x] 1.3 Resource type `pages.page` in the ADR 0017 catalogue with `ResourceFacts` (ancestor walk, highest role, place rules); `cargo xtask arch` rules met; access tests for every row of design.md § Access
- [x] 1.4 `docs/domain-map.md`: the `pages` section (language, events) and the M2 row
- [x] 1.5 Flag-off config test against production compose values

## 2. The document

- [x] 2.1 Block schema allowlist (elements, attributes, link schemes, `ref_type`s, `person` address cleaning) and the update checker over `yrs`; tests incl. forged updates (unknown element, `javascript:` link, oversize update, oversize document, foreign blob)
- [x] 2.2 `pages.sync` (state vector in, missing update out), projections (title, links) in the same transaction, debounced `body` event; concurrency test with two writers
- [x] 2.3 Compaction job (200 updates or 10 min idle) with the snapshot + tail read; race test
- [x] 2.4 Blob references for image/file blocks through the kernel blob store

## 3. Operations

- [x] 3.1 `pages.create`, `pages.get`, `pages.list` (tree, filters, "Shared with you" roots); tests
- [x] 3.2 `pages.update` (revision 409), `pages.move` (fractional positions, cycle and depth 409, leaving `workspace` needs share); tests with undo
- [x] 3.3 `pages.share` / `pages.unshare` (members only, manager or owner), with undo; tests
- [x] 3.4 `pages.trash` / `pages.restore` / `pages.purge`, the 30-day purge job, the workspace purge handler; tests incl. restore after parent purge
- [x] 3.5 `pages.backlinks` (404 for an unreadable record, only readable pages); tests
- [x] 3.6 `pages.page_changed.v1` contract JSON schema in `contracts/`; payload gate passes (no free text)

## 4. Search

- [x] 4.1 `SearchSource` for `pages.page` (title A, text B), `Visibility` answer, backfill, trash and restore; tests incl. the private-page case

## 5. Client

- [x] 5.1 Module `clients/app/src/modules/pages` (routes, nav item, `g p`, `n g` global sequence, flag, i18n en/pt-PT/es-ES/fr); TipTap, yjs, y-prosemirror, fractional-indexing pinned
- [x] 5.2 Tree: places, Shared with you, expand/collapse, drag to reorder and nest, new page and sub-page, trash view with restore and confirmed delete, undo toasts
- [x] 5.3 Editor: schema extensions, Collaboration over `pages.sync` + change-feed pulls, retry with backoff and save status, slash menu, Markdown shortcuts, block handle, sanitised paste, file/image upload
- [x] 5.4 Mentions and embeds: `@` picker by name (members, correspondents, threads, tasks, events, pages); renderers through each module's public read hook; "Something you can't open"
- [x] 5.5 Share popover and "Linked from" section as in the prototype
- [x] 5.6 Phone: tree screen, keyboard formatting bar; Storybook screen stories with mock fetch; `pnpm test:fit` passes at 375 and 1280 in every locale

## 6. Finish

- [x] 6.1 Tracked events and Coverage rows (design.md) in the tracking plan; regenerated
- [x] 6.2 OpenAPI regenerated (`cargo xtask openapi`, events), from CI's `generated-openapi` artifact
- [x] 6.3 Client types regenerated (`pnpm --dir clients api:generate`), from CI's `generated-api-types` artifact
- [x] 6.4 Changelog entry `changelog/unreleased/added/363-pages.md`
- [ ] 6.5 Gates green in CI
- [ ] 6.6 UI verified and recorded at 375 px and desktop (Storybook screen stories)
- [ ] 6.7 `openspec archive add-pages-module --yes` before merge
