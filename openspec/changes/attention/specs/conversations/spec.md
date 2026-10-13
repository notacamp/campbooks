## ADDED Requirements

### Requirement: The need of a conversation
The module SHALL keep at most one open need per conversation, derived from the
newest inbound message sent within
`CAMPBOOKS_CONVERSATIONS__NEEDS_WINDOW_DAYS` (default 14), its decisions, the
policy and the conversation's state. A `sure` need SHALL come only from
answers the policy accepts: `pay` when `asks_action` is accepted with the
`payment_due` reason, else `reply` when `needs_reply` is accepted, else `do`
when `asks_action` is accepted. A `suspicious` answer of true SHALL raise a
`review` need instead of any other. When the `decide` role is configured, a
model answer of true for `needs_reply` or `asks_action` that the policy does
not accept, with a confidence of at least 0.8, where no `sure` need exists
SHALL raise a `review` need with trust `check`. The need SHALL carry the due
date resolved from an accepted `has_deadline` answer, and `urgent` when an
accepted `urgency` answer is "today". Messages from the conversation's own
inbox SHALL never raise one.

#### Scenario: A question from a person
- **WHEN** Marta writes "Could you confirm Thursday's walkthrough? The builders need to know by Wednesday." and the rules accept `needs_reply` and `has_deadline`
- **THEN** the conversation has a `sure` `reply` need due on the Wednesday after the message was sent

#### Scenario: A bill
- **WHEN** an invoice from a service says "Payment due by 30 October" and the rules accept `asks_action` with `payment_due`
- **THEN** the need is `pay`, due on 30 October

#### Scenario: Phishing asks for an action
- **WHEN** a message from a look-alike domain asks the member to verify their mailbox through a link, and the rules answer `suspicious` true and `asks_action` true
- **THEN** the need is `review` and no `do` need exists

#### Scenario: Old mail on the first sync
- **WHEN** a first sync brings a question sent 40 days ago
- **THEN** no need is raised

#### Scenario: Rules only
- **WHEN** no `decide` role is configured
- **THEN** no need has trust `check`

### Requirement: Clearing a need
A need SHALL be cleared, with its cause, when: a message is sent from the
inbox in the conversation after the message that raised it (`replied`; clears
`reply` and `do`), the conversation is archived (`archived`; clears `reply` and
`review`), every message is in the trash or spam or removed (`trashed`; clears
any), or a member resolves it. A new inbound message SHALL derive the need
again from that message (`superseded` for the old one). Changes SHALL come
from the provider or from members alike.

#### Scenario: Replied from the phone
- **WHEN** Ana replies to Marta from Gmail on her phone and the sent message reaches the conversation
- **THEN** the `reply` need is cleared with cause `replied`

#### Scenario: Archiving a bill
- **WHEN** a member archives a conversation with a `pay` need
- **THEN** the need stays open until a member chooses Done or Not needed

#### Scenario: Done in a shared inbox
- **WHEN** Ana chooses Done on a need in an inbox she shares with Leo
- **THEN** the need is cleared for both of them

### Requirement: Need changes are published
Every change to a need SHALL publish `conversations.need_changed` v1
(`contracts/conversations.need_changed.v1.schema.json`) in its transaction,
carrying the workspace, inbox, conversation and message ids, a revision that
grows with every change of that conversation's need, the message's sent time,
the need (reason, trust, due date, urgent) or null, and the cause and the
member when cleared. It SHALL carry no names, addresses, subjects or text, and
SHALL declare an upsert of the conversation within its inbox.

#### Scenario: Cleared by a member
- **WHEN** Ana chooses Done on a need
- **THEN** `need_changed` carries a null need, cause `done`, Ana's user id and a revision one higher than before

### Requirement: Resolving a need
`POST …/conversations/{conversation_id}/need/resolve`
(`conversations.resolve_need`, `conversations:write`) SHALL clear the open
need with outcome `done` or `not_needed`; `not_needed` SHALL also store a
correction of the question that raised it (true to false). `…/need/confirm`
(`conversations.confirm_need`) SHALL turn a `check` need into a `sure` one
and store a correction confirming the answer. `…/need/reopen`
(`conversations.reopen_need`) SHALL undo either, restoring the need as it was
and marking the correction withdrawn. Each SHALL take the `sender` role or
above (a `reader` gets 403 `inbox_read_only`), and SHALL answer 409
`no_open_need` when there is nothing to resolve or confirm, and
`nothing_to_reopen` when there is nothing to reopen.

#### Scenario: Not needed teaches
- **WHEN** a member marks a rules `reply` need Not needed
- **THEN** the need is cleared with cause `not_needed` and a correction of `needs_reply` from true to false overriding `rules` is stored

#### Scenario: Undo
- **WHEN** the member then reopens it
- **THEN** the same need is open again and the correction is marked withdrawn

#### Scenario: It's safe
- **WHEN** a member marks a `review` need for a suspicious message Not needed
- **THEN** a correction of `suspicious` from true to false is stored and no other need is raised for that message

### Requirement: The sender's sentence
The email signals SHALL keep, with a reason whose evidence comes from the
message's text, the sentence of the new part of the message (quoted history
excluded) that holds the evidence, with whitespace collapsed and at most 240
characters, cut at a word. It SHALL be stored with the decision's reasons and
SHALL NOT change any answer.

#### Scenario: A request inside a long paragraph
- **WHEN** a message says "Hope the move went well. Could you confirm Thursday's walkthrough? Thanks!"
- **THEN** the `reply_asked` reason's sentence is "Could you confirm Thursday's walkthrough?"

### Requirement: Due dates from words
The email signals SHALL resolve a deadline's words to a date relative to the
message's sent date (in UTC until accounts carry a time zone): a weekday is
the next such day after the sent date, "today", "tomorrow" and "end of the
week/month" their dates, and a numeric date day-first. Words that do not
resolve SHALL leave the need with no due date and keep the words in the
reason.

#### Scenario: Thursday
- **WHEN** a message sent on Monday 12 October says "by Thursday"
- **THEN** the due date is Thursday 15 October

#### Scenario: Unclear words
- **WHEN** a message says "before the holidays"
- **THEN** the need has no due date and its reason's evidence is "before the holidays"

### Requirement: Needs in summaries
A conversation summary SHALL carry `need` while its need is open: the reason,
trust, due date, whether it is overdue, whether it is urgent, the rule of the
reason that decided it, and the sender's sentence with the sender's name and
the message's sent time. `conversations.list` SHALL accept `ids` (1 to 100
conversation ids, else 400) and return those the caller can see, leaving the
others out.

#### Scenario: Home asks for its rows
- **WHEN** the client lists three conversation ids, one of them in an inbox the caller cannot see
- **THEN** two summaries are returned, each with its `need`
