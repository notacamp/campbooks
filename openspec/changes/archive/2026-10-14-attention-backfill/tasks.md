# Tasks

- [x] Migration: `conversations.needs_backfills` (RLS, maintenance policies, grants), seeded with every inbox (forced RLS on `inboxes` lifted for the seed only)
- [x] `conversations.dispatch_needs_backfills`: every minute, start pending inboxes
- [x] `conversations.backfill_needs`: one page of an inbox's recent conversations or open needs, `needs::refresh` each, record the cursor, enqueue the next page
- [x] Tests: old answered mail gets its needs with rules and model that must not be asked; older than the window gets none; replied, archived and trashed get no Reply; twice changes nothing; another workspace untouched; paging; an inbox that goes; a need its answers no longer support is withdrawn
- [x] The seed as a non-superuser owner (manual check)
- [x] `.sqlx` regenerated, CHANGELOG updated, gates green
- [x] Recording: Home filling in from older demo mail, 375px and desktop
