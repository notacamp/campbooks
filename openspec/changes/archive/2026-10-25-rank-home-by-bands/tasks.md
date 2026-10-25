# Tasks

## 1. Facts
- [x] 1.1 `conversations.need_changed` v1: optional `warning` and `from_person` on the need (`events.rs`, contract, `openapi/events.json`)
- [x] 1.2 `refresh_pass` works them out (`question = suspicious`, `!facts::is_machine_sender` over the message's envelope)
- [x] 1.3 Migration `20261107000100_conversations__need_facts.sql`: `needs.facts_version`; a need announced with older facts is announced again; the seed wakes every open need
- [x] 1.4 Migration `20261107000000_attention__bands.sql`: `warning`, `from_person`, `plan`, `plan_on` on `attention.items`
- [x] 1.5 `project_conversation_need` keeps `warning` and `from_person`; `project_task` keeps `plan` and `plan_on`

## 2. Bands
- [x] 2.1 `attention::domain::bands`: `Band`, `BandRule`, `BandReason`, `band()`, `sort_key()`, unit tests for every rule and boundary
- [x] 2.2 `attention.list` (open): band and reason per item, sorted by band then `sort_key`; `ItemView.band`/`band_reason` (open list only)
- [x] 2.3 Only today's tasks; `tasks_elsewhere`; a task projected before plans were kept stays as shown
- [x] 2.4 List and projection tests; the needs-facts seed test; the contract test sees the new fields
- [x] 2.5 Attention handbook: "Order of the list" and "Tasks on Home"

## 3. Scout
- [x] 3.1 `what_needs_me` passes `band`, `band_reason` and `tasks_elsewhere` (no `urgent`); tool description and handbook line
- [x] 3.2 Test of the tool's result

## 4. Home
- [x] 4.1 `lib/ui` `Item` `eyebrow` slot and `ItemEyebrow`; `QueueGroup` without a signal, `danger` tone; stories and tests
- [x] 4.2 `model.ts` `groupByBand`, `bandOf`, reason sentences; tests
- [x] 4.3 `WhatNeedsYou`: band groups, eyebrow rows (age only at the end), keys in the new order
- [x] 4.4 "Why it's here" in the opened item; Pay and Nudge notes in the opened item
- [x] 4.5 "All N already paid" once under the list; "N more in Tasks" with Open Tasks (`G` `T`)
- [x] 4.6 i18n in `en`, `es-ES`, `fr`, `pt-PT`
- [x] 4.7 Demo data across the four bands; Home stories; WhatNeedsYou tests

## 5. Finish
- [x] 5.1 OpenAPI regenerated (`openapi/openapi.json`: `Band`, `BandReason`, `BandRule`, `ItemView`, `AttentionList`)
- [x] 5.2 Client types regenerated (`schema.gen.ts`)
- [x] 5.3 Tracking plan unchanged (no operation, job, schedule or handler added; checked Coverage)
- [x] 5.4 Changelog entry `changelog/unreleased/changed/home-priority-bands.md`
- [x] 5.5 Gates green in CI
- [x] 5.6 UI verified at 375px and desktop (recording)
