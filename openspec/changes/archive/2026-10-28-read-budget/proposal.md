# Proposal

## Why

Scout's read of a message can fail for good when the model spends its whole
budget thinking. In prod, over 7 days, 6 `complete` answers came back empty
with `finish_reason=length` (5 in `conversations.read_message`, 1 in the
`rule_notices` grouping). The `complete` model is a reasoning model whose
thinking shares `max_tokens` (4096). After `MAX_CALLS = 2` the read is
final, and that thread's "Where things stand" note stays empty with no word
about it.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/375
(found while diagnosing
https://github.com/notacamp/campbooks-cloud/issues/372).

Modules touched: `ai` (the gateway), `conversations`, and the web client
(`clients/app`, the conversations module). No new module, dependency, crate,
migration, event, operation or job. One response field is added to an
existing operation (`conversations.get`).

## What Changes

- **A request may ask for less thinking.** `ChatRequest` gains an optional
  `reasoning_effort` that wins over the role's own. It is sent only to a
  provider that takes the `reasoning` object (new setting
  `CAMPBOOKS_AI__<ROLE>__REASONING=true`, or a role that already has a
  `REASONING_EFFORT`); otherwise it is dropped, so a strict OpenAI-compatible
  server never sees it.
- **The read and the grouping ask for low effort and 8192 tokens.** Chosen
  from a measurement on the eval set (`crates/ai/evals/complete/
  2026-10-28-read-budget.md`): 0 of 50 empty answers at low effort against 3
  of 50 today, half the cost, faster.
- **An empty answer at the limit is asked again once, in the same job,** with
  twice the budget and low effort, before it counts as a failed call.
- **The reader says so when a read failed for good.** `conversations.get`
  carries `need_read_failed`; the note shows one muted line, "Scout couldn't
  read this one.", in all four languages.
- **A failed read is tried again on a newer message.** Already so: each new
  message that raises a need on a flagged message queues its own read with its
  own two calls. Confirmed and written into the spec; no new trigger.

## Non-goals

- No change to what is read or to the instruction.
- No new retry for a failed read of the same message beyond the existing
  second call on open.
- No change to the reply and compile calls (checked: no empty answers in
  prod).
