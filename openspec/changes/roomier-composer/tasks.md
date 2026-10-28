## 1. Server: marks

- [x] 1.1 Shared fixture `crates/modules/conversations/tests/fixtures/marks.json` (doc, marks, html, plain per case); cases listed in design §4
- [x] 1.2 `format.rs`: `[words](url)` (http/https/mailto only), `~~strike~~`, `\` escapes, `Marks::Own` / `Marks::Quoted`, `to_plain`; unit tests + fixture test
- [x] 1.3 `release_send`: render body (`Own`) and quoted history (`Quoted`) separately inside the one `<div>`; plain text via `to_plain(body)`; tests incl. the quoted `[your bank](…)` scenario

## 2. Client: the editor

- [x] 2.1 `MessageComposer/marks.ts`: `toMarks` / `fromMarks` (escapes, one list level), vitest against the shared fixture
- [x] 2.2 TipTap editor with the fixed schema (design §1) replacing the textarea; set once at mount, server echo replaces only when it differs; paste keeps only the schema
- [x] 2.3 Toolbar: Bold, Italic, Strikethrough, Link, Bullets, Numbers, Quote with key tooltips; More menu below 32rem of composer width (container query); active state on marks under the cursor
- [x] 2.4 Link dialog: words + address, http/https/mailto only with the refusal message; paste a URL over a selection links it
- [x] 2.5 Shortcuts `compose.strike|link|bullets|numbers|quote|expand` in the catalog (+ `?` sheet labels); TipTap's own bindings for these off
- [x] 2.6 Room: fields at body size, 40px rows, body `min-height: 15.5em`, sticky footer inside the reply slot's scroller
- [x] 2.7 Expand / Shrink (⇧⌘E, Esc) as a native `<dialog>` sheet over the one composer instance (state, cursor and history kept); hidden below 48rem
- [x] 2.8 i18n en / pt-PT / es-ES / fr; `pnpm test:fit` at 375 and 1280; Storybook stories (idle + interactive), component tests

## 3. Finish

- [ ] 3.1 OpenAPI and client types regenerated (no change expected)
- [ ] 3.2 Tracking plan regenerated (no change expected)
- [x] 3.3 Changelog entry `changelog/unreleased/changed/390-roomier-composer.md`
- [ ] 3.4 Gates green in CI
- [ ] 3.5 Recording at 375 px and desktop
- [ ] 3.6 `openspec archive roomier-composer --yes` before merge
