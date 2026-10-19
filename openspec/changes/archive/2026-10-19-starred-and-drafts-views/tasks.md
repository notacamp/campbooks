## 1. API

- [x] 1.1 `MailView::Starred` (`view=starred`), no new index, `ListQuery` docs
- [x] 1.2 Tests: inbox and archive, unstarred/spam/trash left out, the cursor pages through

## 2. Client

- [x] 2.1 `starred` joins the views; `Place` adds `drafts`, `ConversationPlace` for the list functions
- [x] 2.2 Sidebar and chips: Sent, Pinned, Drafts, Archive, Spam, Trash; G 9 and G 0
- [x] 2.3 `DraftsScreen` and `DraftsList`: rows, open, Delete draft with Undo, empty state, 50 note
- [x] 2.4 The active chip scrolls into view (#279)
- [x] 2.5 en, pt-PT, es-ES and fr; stories and unit tests

## 3. Wrap-up

- [x] 3.1 OpenAPI regenerated, client types regenerated
- [x] 3.2 Changelog entry file
- [x] 3.3 Recording scenario `starred-drafts`
- [x] 3.4 Gates green, UI verified at 375px and desktop
