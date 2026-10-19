## 1. Data

- [x] 1.1 Migration: `conversations.last_sent_at`, `conversations.spam` (with its check), the backfill
- [x] 1.2 Concurrent partial indexes for Sent, Spam, Trash and Archive
- [x] 1.3 `recount` keeps both columns (non-macro query)

## 2. API

- [x] 2.1 `conversations.list` `view`, the deprecated `archived`, `invalid_view`
- [x] 2.2 One statement from the view's fragments; Sent's cursor on `last_sent_at`
- [x] 2.3 Summaries: `trashed`, `spam`, `last_sent_at`, `sent_to`, `replied_since_sent`
- [x] 2.4 Images in spam held back, whoever asks (`images_held: spam`)
- [x] 2.5 Tests: each view, ordering, paging, the alias, isolation, recount, the backfill

## 3. Client

- [x] 3.1 Places: streams and views, routes, the sidebar with its divider, the phone chips
- [x] 3.2 G 5 to 8, Delete (#) and Report spam (!) with Undo, the way back per place
- [x] 3.3 Sent rows, empty views, the Spam and Trash banners, no reply or Scout there
- [x] 3.4 en, pt-PT, es-ES and fr; stories and unit tests

## 4. Wrap-up

- [x] 4.1 OpenAPI regenerated, client types regenerated
- [x] 4.2 CHANGELOG updated
- [x] 4.3 Gates green, UI verified at 375px and desktop
