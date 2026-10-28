# Design

## The condition reads addresses only

`Condition::MemberNotInTo` holds when `Facts.member_not_in_to` is true. The
fact is `signals::facts::addressed_to_inbox(envelope) == Some(false)`: the
inbox is known and none of its addresses is in `To`. An inbox that is not known
fails it (the condition fails closed). It is worked out in the three places
that build the facts:

- on arrival (`apply_changes::taught_outcome`), from the message's `To` and
  the inbox's address, looked up only when an acting rule asks for it;
- for existing mail (`Candidate`, marked by `mark_addressing`, one runtime query
  per window), so previews and runs agree with arrival;
- by the `TaughtRules` strategy, from the envelope.

Weight 1, like a domain or a stream: it never narrows by itself, so a rule
needs a sender (or list, header or subject) next to it.

## The action answers like the member did

`Action::WaitOnAddressed` fills its own slot (`wait`), so it can share a rule
with a `needs_reply` or `asks_action` answer. It is valid only next to
`member_not_in_to` (else it would wait on whoever the member wrote to).

The engine only says the rule wins (`Outcome.wait`). The person is a fact of
the message's headers, so it is resolved where the answers are stored
(`rules::waits::answers`, on arrival and when a run applies the rule): the
single person of `facts::awaitable` who is also in `To`. Zero or several give
no answer. The two stored answers are `taught` decisions, like the member's
corrections would be: `next_move` = `someone_else` and `awaited` = `pN`, whose
stored option holds the person's address so a later read can tell the headers
still name them. Revert deletes them like any answer of the rule.

## The wait is the usual wait

`needs::waiting` now reads the move answers whoever gave them: the model's, or
a taught rule's, which `taught_first` has already put in front of the model's.
A taught `next_move` that is not `recipient` is also what `moved_on` reads, so a
model's own "needs a reply" or "asks for an action" is nothing, exactly as if
the policy had accepted the model's `next_move`. The need is `sure` (a taught
answer acts) with basis `taught`. Nothing else changes: the wait is raised only
when the switch of whose move it is (`CAMPBOOKS_CONVERSATIONS__MOVES__ENABLED`)
is on; never on the inbox itself, never on a machine sender, never on a sorted
stream other than Primary; its nudge time, heard-back, and re-arming when the
member writes to them are the code of every other wait.

### What wins

- **Another taught rule's `needs_reply` or `asks_action` yes for the same mail
  wins.** `raise_with` raises a Reply or Do from a taught answer first, and a
  wait only when nothing else is raised. The member's own "reply" is never
  silently turned into a wait. Both rules still apply (their answers are
  stored), so removing the reply rule leaves the wait.
- **A taught `needs_reply` or `asks_action` no** leaves the wait standing: the
  mail asks nothing of the member and someone else has the move.
- **The model's `needs_reply` or `asks_action` yes loses** to the rule: the rule
  is the member speaking, and its answer replaces the model's `next_move`.
- Two wait rules for one message: the engine gives the slot to the more
  specific rule, then the later confirmed.
- Suspicious mail, or mail failing its sender checks, never gets a wait from a
  rule (the engine drops it).

## The offer

`GET …/rule-suggestions?action=waiting` answers a suggestion only when:

1. the conversation's newest inbound message went to the member without them in
   `To` (the candidate fact), so the rule would match the next one like it;
2. the member's own answer on it, the verdict stored with a person, names exactly
   the person the rule would name (`rules::single_addressed`), so the rule does
   what they did;
3. the member's `next_move` → `someone_else` corrections (their own
   `corrected_by`, not withdrawn, on messages from this sender in this inbox)
   number at least two, this one included. The tx runs in the member's
   workspace, so row-level security bounds the count. A runtime query, so it needs
   no entry in the `.sqlx` offline cache.

Fewer than two is `too_few`; anything else that fails is `not_teachable`. A
dismissal reuses `dismiss_rule_suggestion` and `rule_dismissals` with the
suggestion's key (30 days, and for good after the second), so "Ask me each time"
needs nothing new. Undo (`reopen_need`) withdraws the corrections and clears the
person, which empties the count and the match, so a later ask finds nothing, and
the client cancels an offer not yet shown (`AheadOffer.cancel`).

The response carries `wait_days` (the configured working days a wait lasts) only
for this offer, so the sub-line is true wherever it is configured.

The preview counts the mail the rule would look at; whether each message has a
single person in `To` is settled when the rule acts.

## Client

`useAheadOffer` gets the action `waiting`; Home's `onWaiting` and the reader's
call `ahead.offer(conversation, { kind: "waiting" })` beside the nudge dialog.
The sentence for the rule is its own string in four locales.

## Tracking

No operation, job, schedule, handler or event is added. The tracked snapshot of
a rule lists its actions by kind through the plan's `conversations.rule_action`
enum, which is a workbook edited with the plan tool. This change leaves the new
kind out of that list rather than hand-edit the generated files; a follow-up
patch adds the value (`wait_on_addressed`) to the plan.

## AI use

None added: the rule is deterministic. A member undoes any answer.

## Migrations

None. Rules store conditions and actions as JSON, their MAC covers the JSON, and
the decisions and corrections tables take the same questions as before.
