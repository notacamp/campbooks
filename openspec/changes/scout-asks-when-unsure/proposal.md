# Scout asks when it is unsure

Modules: `campbooks-ai` (`decide` facts: one more header fact), `conversations`
(the policy's sure bars, the Cc veto, a switch for whose move it is, the
question's people, "someone else has the move"), and the `attention` and
`conversations` client modules (Home's row and the reader's note). Tracking:
https://github.com/notacamp/campbooks-cloud/issues/372, part of #164. No new
module and no cross-module dependency. Builds on whose move it is
(campbooks-cloud#293, public PR #616).

## Why

A message from a colleague to a third party, with the member only in Cc,
became an open "Reply · sure" need. The model had answered `needs_reply` yes
at 0.59. The policy acts on `needs_reply` from 0.51 and every accepted reply
is raised as `sure`, so a guess was shown as a fact. Whose move it was never
ran in prod, because its switch is the action catalogue's, and the catalogue
waits for the owner's labels.

The measurement the owner asked for
(`crates/ai/evals/decide/mail-triage/reports/2026-10-25-confidence-bands-and-moves-cost.md`,
320 fictional items, so a floor) says the same about the yes side:

| P(yes) | `needs_reply` right | `asks_action` right |
| --- | --- | --- |
| 0.5 to 0.6 | 25% of 4 | 0% of 2 |
| 0.6 to 0.7 | 33% of 6 | 25% of 4 |
| 0.7 to 0.8 | 50% of 4 | 40% of 10 |
| 0.8 to 0.9 | 76% of 17 | 76% of 17 |
| 0.9 to 1.0 | 89% of 46 | 99% of 138 |

## What changes

- **A guess is never shown as sure.** Acting and being sure are split. A
  model's `needs_reply` yes raises a `sure` Reply only from 0.9, and only when
  the message is not answered suspicious (97.6% of 42 on the set). An
  `asks_action` yes raises a `sure` Do from 0.8 (96.8% of 155). From 0.8 to
  the bar it is a question to the member. Below 0.8 it is nothing: an honest
  empty beats a wrong card. Taught rules keep acting as they do.
- **Cc only lowers trust.** When the inbox is not in `To` (copied, or not
  named), a model's Reply or Do is a question, unless the policy accepted
  `next_move` `recipient`. A fact of the headers, no content.
- **Whose move has its own switch.**
  `CAMPBOOKS_CONVERSATIONS__MOVES__ENABLED` (off by default) gates asking
  `next_move` and `awaited`, raising waits, and the move backfill. The action
  catalogue's switch gates only the catalogue. Switching moves on later asks
  the mail of the last 14 days once.
- **The question offers the moves.** When such a question is about mail that
  went to other people, Home's row and the reader's note say who wrote to
  whom, and offer Nudge them, Reply, or Nothing for me. Each answer is a
  correction of the right question. "Someone else has the move" makes the
  conversation wait on that person at once.
- **Open needs are worked out again, once,** under the new policy, from the
  answers already stored. No model is asked.

## Non-goals

- Changing the operating points the committed eval report sets (`stream`,
  `has_deadline`, `suspicious`, and where `needs_reply` and `asks_action` act).
  Only the two sure bars are new.
- Measuring `next_move` and `awaited`. They stay recorded; the report has no
  labels for them.
- Teaching a rule after the same answer twice. It is left as a task
  (`tasks.md`, section 6) for a change of its own.
- A second channel, new tracked events, or any change to what the action
  catalogue offers.
