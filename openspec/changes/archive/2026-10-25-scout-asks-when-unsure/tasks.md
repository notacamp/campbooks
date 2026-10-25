# Tasks

## 0. Gate

- [x] 0.1 The owner approved the visual prototype (option A: the question card offers the moves; 375 and desktop)
- [x] 0.2 The report's scale checked against the server's: `confidence` is the answer's probability after the configured temperature (default 1.0, none set in the deploy config), so the bars read on the report's raw scale (design.md)

## 1. Policy and facts

- [x] 1.1 `Policy::sure_bar` (`needs_reply` 0.9, `asks_action` 0.8), cited to the 2026-10-25 report in the table and the module doc; the operating points and the test that mirrors the committed baseline unchanged
- [x] 1.2 `facts::addressed_to_inbox` (headers' addresses only), unit tests; the facts-read-no-content test still passes
- [x] 1.3 `needs::ask`: sure only from the bar, not suspicious, inbox in `To` (or `next_move` `recipient` accepted); a question from the check floor; nothing below it; unit tests for every row of the table

## 2. Whose move has its own switch

- [x] 2.1 `CAMPBOOKS_CONVERSATIONS__MOVES__ENABLED` (`MoveSettings`, off by default); the catalogue's switch gates only the catalogue; nothing validated at start
- [x] 2.2 The decide pass (`asked`, `moves`, the moves-only backfill pass), `NeedSettings::waits` and the backfill read the new setting
- [x] 2.3 `Background::schedule_with`; `DispatchNeedsBackfills { moves }` puts finished backfills made without the moves back to pending once (`needs_backfills.moves_asked`); nothing with the switch off
- [x] 2.4 `deploy/.env.example` documents the key

## 3. The question's people and the answer

- [x] 3.1 `NeedView.unsure` for a question about a reply or an action on mail with other people
- [x] 3.2 `wait_on` on `conversations.resolve_need`: `not_a_party`, two corrections, the verdict kept with the person, a `waiting` need on them; reopen withdraws both
- [x] 3.3 Migration `20261108020000_conversations__unsure_asks.sql`: columns, `moves_asked`, clocks for every open need (the re-derivation, no model)
- [x] 3.4 HTTP tests: the copy is a question, Nudge files two corrections and waits, Undo, `not_a_party`

## 4. Client

- [x] 4.1 `needs.ts`: the card's sentences and the row kind; four locales, full diacritics; `pnpm test:fit`
- [x] 4.2 Home's row and the reader's note: the card, keys 1 to 3, Later, Ask Scout, "How Scout decided"; the nudge dialog over the wait; Undo
- [x] 4.3 Screen stories (desktop and 375, en), each with a play-less twin for the recording

## 5. Close

- [x] 5.1 OpenAPI, client types and tracking plan regenerated (no plan change), changelog entry file
- [x] 5.2 Gates green; UI verified at 375px and desktop; the recording left to the maintainer (`## Recording` in the PR)
- [x] 5.3 Change archived into `openspec/specs/`

## 6. Follow-up (not in this change)

- [ ] 6.1 Teach once: after the same answer for the same sender in this situation twice, offer a smart rule ("When Ana writes to someone else with you in Cc, should Scout wait on them for you?"). It needs a rule condition on "the inbox is not in `To`" (an address-only fact), the second answer counted per sender and kind in `rule_notices`, and the offer in the existing notice. Left out so this change stays whole; to be filed as its own issue
