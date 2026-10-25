# Tasks

## 1. lib/ui
- [x] 1.1 `Item`: `kind` draws the kind's square (`KindIcon` `md`) in place of an avatar; an eyebrow without a signal has no dot; stories and tests
- [x] 1.2 `KindDot` (filled, or a ring with a label); `QueueGroup` without a count; stories and tests

## 2. schedule
- [x] 2.1 `day.ts`: `nextUp`, `laterToday`, `comingUp`, `offersAdd`, `pushTarget`, `joinUrl`, `dayFacts`; unit tests (121 minutes, started 5 minutes ago, all-day, declined, hold, Coming up order, limit and the add rule)
- [x] 2.2 `useHomeDay` (rows in words, actions, Add to my calendar through `useEventActions`), exported from the module's index; `useClock` shared with the Calendar screen
- [x] 2.3 i18n in `en`, `es-ES`, `fr`, `pt-PT` (countdowns, Add to my calendar's name)

## 3. Home
- [x] 3.1 Next up group above Now; today's events at the end of Today; Today for events alone
- [x] 3.2 ↑/↓ over items and events; Enter opens an event in Calendar; Join and Open email on 1 and 2
- [x] 3.3 The hero's facts; Coming up first in the aside, with Open Calendar (`G` `S`)
- [x] 3.4 i18n in `en`, `es-ES`, `fr`, `pt-PT`
- [x] 3.5 `WhatNeedsYou` calendar tests; demo data (`test/demo/home.ts`); Home screen stories (desktop, selected, add, 375px)

## 4. Finish
- [x] 4.1 OpenAPI unchanged (no endpoint)
- [x] 4.2 Client types unchanged
- [x] 4.3 Tracking plan unchanged (no operation, job, schedule or handler; checked Coverage)
- [x] 4.4 Changelog entry `changelog/unreleased/added/home-calendar-in-the-list.md`
- [x] 4.5 Gates green in CI
- [x] 4.6 UI verified at 375px and desktop (recording)
