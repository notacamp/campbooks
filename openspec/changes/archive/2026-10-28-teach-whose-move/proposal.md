# Teach once from Scout's whose-move answers

Modules: `campbooks-ai` (`decide::taught`: one condition, one action),
`conversations` (rules, needs, rule suggestions) and the client (Home and the
reader). Tracking: https://github.com/notacamp/campbooks-cloud/issues/378. No
new module and no cross-module dependency. Builds on "Scout asks when it is
unsure" (campbooks-cloud#372, public #679) and on teach once (campbooks-cloud#125).

## Why

When Scout isn't sure whose move it is, the member answers "Nudge Ana…". The
answer is stored and the conversation waits on Ana. When Ana's colleague writes
to someone else with the member in Cc again and again, the member gives the
same answer again and again. The second time is the moment to offer a rule.

## What changes

- **A condition, `member_not_in_to`.** None of the inbox's addresses is in the
  message's `To`. It reads addresses only (ADR 0018).
- **An action, `wait_on_addressed` ("Wait on them").** On a matching message the
  rule answers as a taught source what the member's own choice filed:
  `next_move` is `someone_else` and `awaited` is the one awaitable person in
  `To`. With none or several it answers nothing and the card asks as before.
  The conversation then waits on that person at once, through the same wait
  machinery as any other, with its nudge time and guards.
- **The offer.** After "Nudge {party}…", from the second time for the same
  sender, Home and the reader offer: "When Ana writes to someone else with you
  in Cc, should Scout wait on them for you?" with **Yes, always** (opens the
  smart-rule confirmation, which alone saves) and **Ask me each time**
  (dismisses it). Undo of the answer cancels the offer.
- **The count.** `conversations.suggest_rules` learns `action=waiting`. It
  offers only when the member's active, not-withdrawn `next_move` →
  `someone_else` corrections on mail from this sender number two or more.

## Non-goals

- Sending anything on the rule's behalf: the nudge stays the member's.
- Teaching from "Reply" or "Nothing for me" (a `needs_reply` rule already
  exists for those).
- A new column, table or migration.

## Impact

OpenAPI (`TaughtAction`, `RuleCondition`, `RuleAction`, `RuleSuggestions.wait_days`)
and the client types follow. No migration, no scope, no new operation, job or
event. The tracking plan's `conversations.rule_action` enum does not gain the new
value in this change (see design.md).
