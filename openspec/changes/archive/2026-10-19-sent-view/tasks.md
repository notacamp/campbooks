# Tasks

## 1. Server

- [x] 1.1 Migrations: `last_sent_at` with backfill; the Sent list index
- [x] 1.2 `recount` maintains `last_sent_at`
- [x] 1.3 `conversations.list` takes `sent`; kinded cursor; `last_sent_at` and `sent_to` in every summary query
- [x] 1.4 Tests: started and archived, trash and spam, order, paging, narrowing, visibility, recompute, a reply brings it back
- [x] 1.5 `openapi/openapi.json`, `schema.gen.ts` and `.sqlx` updated

## 2. Client

- [x] 2.1 Sent item (G then 5), phone chip, `NavItem` `apart` variant with a story
- [x] 2.2 Sent route reusing the inbox screen and reader; rows "To <name>", "Replied" chip, empty state
- [x] 2.3 i18n in four locales; model and route tests
- [x] 2.4 Demo data (fake Gmail), `sent` recording scenario

## 3. Docs

- [x] 3.1 CHANGELOG
