## 0. Before

- [x] 0.1 Rebase onto `next` once part 1 (#690) is merged (26bff87c2)
- [ ] 0.2 Check how the client learns part 1's flag; record it in design.md if it differs

## 1. Save to page

- [ ] 1.1 `SaveToPageDialog` (steps, picker with backlinks first, choices, attachments, `⌘↵`)
- [ ] 1.2 Reader More menu entry and the `pages.save_thread` shortcut with its hint
- [ ] 1.3 Live block, copy builder and attachments in one sync
- [ ] 1.4 The readers line for a copy (`pages.get_sharing`)
- [ ] 1.5 Scout's summary hand-off (hidden when Scout is unavailable)
- [ ] 1.6 Undo toast (remove inserted blocks, detach files); attach refusals said by name (403, 404, 413)

## 2. Scout

- [ ] 2.1 `PageChangeCard`: target, readers, Notion, rendered Markdown, sources, actions; no "Add to page" until all of it shows
- [ ] 2.2 "Edit first" and the added state with Open page and Undo
- [ ] 2.3 `RUNNABLE` gains `pages.append` and `pages.create`
- [ ] 2.4 "Ask Scout" in the page's More menu; `SCOUT_CHAT_ABOUT` gains `pages.page`

## 3. Finish

- [ ] 3.1 i18n en, pt-PT, es-ES and fr; `pnpm test:fit`
- [ ] 3.2 Stories and unit tests
- [ ] 3.3 Changelog entry `changelog/unreleased/added/364-pages-from-mail-and-scout-ui.md`
- [ ] 3.4 Gates green in CI
- [ ] 3.5 Recording at 375 px and desktop (Storybook)
- [ ] 3.6 `openspec archive pages-from-mail-and-scout-ui --yes` before merge
