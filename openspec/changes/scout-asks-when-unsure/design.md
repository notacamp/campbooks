# Design

## The scale of the bars

The report's bands are on the raw `noul` probability of "yes". The server
stores and compares `Answer::confidence`, which `decide::finish` derives as
the largest of the answer's probabilities after re-tempering them with the
configured temperature (`CAMPBOOKS_AI__DECIDE__CALIBRATION__NOUL`, default
1.0). For a "yes" that is P(yes) when the answer is yes. With no temperature
set, which is the shipped default and what the deploy config carries, the two
scales are the same number and the report's bars apply as measured.

If an operator sets a temperature, the bars no longer read on the report's
scale. The report fitted T = 0.69 for `needs_reply` and 0.64 for
`asks_action`; with those, the same bars measure worse (86.4% at 0.9 for
`needs_reply`) and the sure bar would have to move to about 0.97. So the bars
are constants of the policy, cited to the report, and a temperature is a
reason to measure again, not to rescale by hand.

## The bars

`Policy::sure_bar(question)`: `needs_reply` 0.9, `asks_action` 0.8. The
operating points where an answer may act (0.51 and 0.58) do not change: the
policy test that mirrors the committed baseline keeps passing, and the
disclosure in "How Scout decided" still shows them. The bars are a second
number per question, above the operating point.

| `needs_reply` yes | Today | Now |
| --- | --- | --- |
| below 0.8 | `sure` Reply (acts from 0.51) | nothing |
| 0.8 to below 0.9 | `sure` Reply | question |
| 0.9 and over, a `suspicious` yes on the message | `sure` Reply | question |
| 0.9 and over, not suspicious | `sure` Reply | `sure` Reply |

`asks_action`: the bar and the check floor are both 0.8, so a yes is a `sure`
Do from 0.8 and nothing below it; it becomes a question only when the Cc veto
or an accepted `suspicious` applies. That is what the report supports: from
0.5 to 0.8 the yes was right 0% to 40% of the time on 16 answers.

Only these two bars are new. `stream` (0.49), `has_deadline` (0.51) and
`suspicious` (0.54) interact with the needs only through the stream gate and
the warning, and their operating points are unchanged.

A taught rule (`Source::Taught`) and the facts' "no" keep acting as now.

## Cc only lowers trust

`facts::addressed_to_inbox(envelope)` is `Some(true)` when one of the inbox's
addresses is in `To`, `Some(false)` otherwise, `None` when the inbox is
unknown. It reads addresses only. In `needs::ask`, a model's Reply or Do is a
question when it is `Some(false)`, unless the policy accepted `next_move`
`recipient` (never today). `copied_only` (the hint the model is told) stays as
it is; this is the same fact used for trust.

## The question's people

`NeedView.unsure` is present only for a question about a reply or an action
(`review`, trust `check`, question `needs_reply` or `asks_action`) on mail
whose `awaitable` people (the facts' list: not the inbox, not the sender, no
machine addresses, at most six) are not empty. It holds those people, whether
the inbox is in `To`, the one natural party (the person the model's `awaited`
answer named when its stored option still names that address, else the only
other person in `To`), the days since the email when none of them wrote to the
inbox since (a header fact), and the confidence and sure bar in whole percent
for "How Scout decided". No text of the email is read.

## Someone else has the move

`POST …/need/resolve` takes an optional `wait_on` (an address) with
`not_needed`. Why not a new verdict: the verdict is part of the tracked
`NeedResolved`/`NeedConfirmed` snapshots, whose enum lives in the tracking
plan; the member's act here is "this is not mine", which `not_needed` already
says, and the operation, its scope and its Coverage row stay as they are.

- The address must be one of the email's `awaitable` people (409
  `not_a_party`), and the need must be a `check` question about a reply or an
  action (409 `no_open_need`).
- Two corrections are filed, of the right questions: `next_move` (from what the
  model said, `recipient` if it was not asked, to `someone_else`) and
  `awaited` (from `none` or its stored answer to the option `pN` that names
  the person, in the facts' order). The second is kept in
  `needs.verdict_extra_correction_id` so Undo withdraws both.
- The verdict is stored with the person (`needs.verdict_awaited_*`). When the
  need is worked out, `not_needed` with a person is read as a wait, not a
  clearing: `Need::waiting_on` makes it a `waiting` need on that person, `sure`,
  basis `member`, question `next_move`, with the nudge time worked out as for
  any wait. A newer message replaces it, like any need.
- **It becomes a wait right away.** The nudge is optional: Nudge… opens the
  existing nudge dialog over the wait, and closing it without sending leaves
  the conversation in Waiting on others, where its nudge time comes round and
  Scout offers the nudge again. The alternative (a wait only once a nudge is
  sent) would leave the answer unrecorded when the member changes their mind
  halfway, and would make "Nudge" the only way to say someone else has the move.
- Undo (`reopen`) clears the person, withdraws both corrections and brings
  the question back.

Choosing Reply files the existing `needs_reply` confirmation (Yes) and opens
the reply dialog. Nothing for me is the existing `not_needed`, a correction of
`needs_reply`.

## Whose move has its own switch

`MoveSettings { enabled }` (`CAMPBOOKS_CONVERSATIONS__MOVES__ENABLED`, off by
default) replaces the action catalogue's switch in four places: asking the
move questions with triage, raising waits (`NeedSettings::waits`), the
moves-only backfill pass, and the backfill reset below. The catalogue's switch
now gates only `action_questions`, the rated actions and the attachments read.
Nothing validates the new key at start: off is the default, and a config check
must never stop the server for a flag that is off (campbooks-cloud#641, #646).

### Moves after the fact

The needs backfill only visits inboxes with a pending row in
`needs_backfills`, and rows finish `done`, so a seed made while the switch is
off would be spent. `needs_backfills.moves_asked` records whether a run asked
the moves (set on its first page from the setting). The dispatcher
(`DispatchNeedsBackfills { moves }`, scheduled with the setting through
`Background::schedule_with`) puts finished rows with `moves_asked = false`
back to pending when the switch is on, once; with it off it does nothing.
The run then asks the newest inbound message of each recent conversation with
no open need and no checked `next_move`, within the 14-day windows and the
workspace's daily AI limit, as before.

## Open needs are worked out again

Policy over stored answers: a migration gives every open need a clock due now,
and `conversations.wake_needs` works each one out (`needs::refresh`), as
`need-facts` did. A `sure` Reply or Do the new policy does not stand behind is
announced as a question (`review`, `check`); one whose answer is below the
check floor, with no member's confirmation behind it, is withdrawn and cleared
as `superseded` (the existing cause for a need its own message no longer
raises). A need the member confirmed keeps its place. No model is asked. A
second run changes nothing.

## The card

Home's row and the reader's note read `need.unsure`:

- "**Ana Costa** wrote to **Águas do Ribatejo**. You're in Cc." (or
  "…and you." when the inbox is in `To` with others), with the days unanswered
  when the headers say so, and the chip "Scout isn't sure".
- Actions 1 to 3 in one style: Nudge {party}… (hidden without an obvious
  party, or in a read-only inbox), Reply to {first name}…, Nothing for me; then
  Later (S) and Ask Scout (`.`).
- "How Scout decided" adds the confidence, the sure bar and, when it applied,
  "You're in Cc, not To.".
- With the inbox the only `To` and nobody else on the email, the question is
  today's Yes / No card.

Buttons stack full width below 40rem with 44px targets; every string has the
four languages and passes `pnpm test:fit`.

## AI use

Role `decide`, unchanged: no call is added. With the switch off the questions
asked are exactly the triage questions. The member undoes any answer (Z or the
toast's Undo).

## Tracking

No new operation, job, schedule, event or handler: `conversations.resolve_need`
and `conversations.reopen_need` already have their rows, and the verdict the
member gives is `not_needed`. `DispatchNeedsBackfills` keeps its name and its
Coverage row.

## Migrations

`20261108010000_conversations__unsure_asks.sql`: three nullable columns on
`needs`, one boolean on `needs_backfills`, and the clocks. No check
constraint changes (the verdict stays `not_needed`).
