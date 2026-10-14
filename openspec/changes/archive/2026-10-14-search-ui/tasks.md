## 1. Spec

- [x] 1.1 Proposal, design (Decision 4 referenced), spec delta
- [x] 1.2 `openspec validate --all --strict`

## 2. `lib/ui`

- [x] 2.1 `Highlight` (segments, `<mark>`, no HTML injection); story, test, axe
- [x] 2.2 `SearchField` (field, removable chips, clear, shortcut hint); story, test, axe
- [x] 2.3 `ConversationRow`: node subject and snippet, `option` mode; story, test
- [x] 2.4 Public `Overlay`; ⌘K `onSearch` row; search icon
- [x] 2.5 `ui` copy in en, pt-PT, es-ES, fr

## 3. `search` client module

- [x] 3.1 `search.query` hooks (instant and paged), query/chip helpers, recent searches
- [x] 3.2 `/` shortcut (`search.open`), overlay with keyboard handling
- [x] 3.3 Results page, route, reader beside, inbox shortcuts, sort toggle, paging
- [x] 3.4 States: recents, tips, no results, indexing, errors
- [x] 3.5 Phones: top-bar button, full-screen page, reader as its own screen
- [x] 3.6 Copy in en, pt-PT, es-ES, fr

## 4. Wiring and checks

- [x] 4.1 App shell, `conversations` exports
- [x] 4.2 Tests, stories, axe light and dark; client gates
- [x] 4.3 Recording at 375 px and desktop; changelog; archive
