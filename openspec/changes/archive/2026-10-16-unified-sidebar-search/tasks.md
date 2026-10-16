# Tasks

## 1. Menu

- [x] 1.1 CommandMenu takes a section hook (`useSection`) listed above the commands, sharing one cursor, with status slot, chips and a ⌘↵ handler; the "Search conversations for" row is removed
- [x] 1.2 Footer hint bar: move, open, all results, close
- [x] 1.3 `modules/search` supplies the conversation section (instant hits, chips, All results row, states, recent searches); SearchOverlay is deleted

## 2. Shell

- [x] 2.1 The sidebar keeps one "Search or ask" field hinting ⌘K and `/`; the "Search conversations" item is removed
- [x] 2.2 `/` opens the menu on desktop and the results page below 48rem; ⌘K opens the same menu

## 3. Tests

- [x] 3.1 CommandMenu tests and a story for a section: order, one cursor, ↵, ⌘↵, chips, keep-open rows
- [x] 3.2 Search module tests for the section (the overlay's scenarios) and the app test for `/` and ⌘K opening the same menu

## 4. Finish

- [x] 4.1 OpenAPI and client types unchanged (no API change)
- [x] 4.2 CHANGELOG updated
- [x] 4.3 Gates green
- [x] 4.4 UI verified at 375px and desktop, with a recording
