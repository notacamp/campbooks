# Tasks

## 1. Data
- [x] 1.1 Migration (conversations): `needs.lapses_at`; `need_summaries` dates and `dates_read_at`; `need_clocks` with row-level security and its maintenance read policy; widen the question and cleared-cause checks `NOT VALID`; queue every inbox for the needs backfill
- [x] 1.2 Migration (conversations): validate the widened checks
- [x] 1.3 Migration (attention): `items.lapses_at`

## 2. Conversations
- [x] 2.1 Vocabulary: `still_to_do`, `still_needs_reply`, cleared cause `lapsed`
- [x] 2.2 `settle` with a clock (lapsed, still needed?) and the next wake time; unit tests (lapsed, stale becomes a question, confirmed stays, Pay never lapses, a warning unchanged, dated asks unchanged)
- [x] 2.3 The refresh fills the need's dates from the message's summary row, writes the clock row, carries `lapses_at` on `need_changed`
- [x] 2.4 `summarize_need`: dates in the schema and the instruction, no time relative to now, validation (bad, early, absurd dates dropped), stored with `dates_read_at`, the need worked out again; re-read once for summaries from before; tests
- [x] 2.5 Backfill: queue summaries never dated
- [x] 2.6 `dispatch_need_clocks` and `wake_needs`; `architecture.toml` entry; access test lists the policy; tests
- [x] 2.7 Confirm and Not needed on a still-needed question store no correction; tests
- [x] 2.8 `NeedView.sent_at`, `NeedView.lapses_at`, `ConversationSummary.lapsed_need`

## 3. Attention
- [x] 3.1 Project `lapses_at`; list hides lapsed items, keeps overdue bills, leaves lapsed out of Done today; tests

## 4. Client
- [x] 4.1 Still-needed sentences with how long ago, "No longer needed" toast, no teach-once after it; tests
- [x] 4.2 "Expired" chip in the list and the reader; tests
- [x] 4.3 Strings in en, pt-PT, es-ES and fr
- [x] 4.4 Fake `complete` answers dates; demo mail and a recording scenario

## 5. Ship
- [x] 5.1 OpenAPI regenerated, events catalogue and contract updated, client types regenerated
- [x] 5.2 CHANGELOG updated
- [x] 5.3 Gates green in CI
- [ ] 5.4 UI verified at 375px and desktop (recording by a maintainer)
