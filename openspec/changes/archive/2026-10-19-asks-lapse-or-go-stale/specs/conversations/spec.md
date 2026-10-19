## MODIFIED Requirements

### Requirement: The need of a conversation
The module SHALL keep at most one open need per conversation, derived from the
newest inbound message sent within
`CAMPBOOKS_CONVERSATIONS__NEEDS_WINDOW_DAYS` (default 14), its decisions, the
policy and the conversation's state. A `pay` need SHALL be raised only when
an answer to `money` is `owed` and either the policy accepted it or it is the
model's with a confidence of at least 0.8, no accepted `money` answer says
otherwise, and the message's accepted `stream` answer is not `newsletters`;
it SHALL have trust `check` and question `money` (Campbooks cannot see the
bank, so it asks whether it is paid). A message whose accepted `stream`
answer is `receipts` SHALL raise no other need, unless a rule a member taught
says it needs a reply or an action. Otherwise a `sure` need SHALL come only
from answers the policy accepts: `reply` when `needs_reply` is accepted and
true, else `do` when `asks_action` is accepted and true. A `reply` or `do`
need, `sure` or `check`, SHALL be raised only when the message's accepted
`stream` answer is `primary`, or, when it is `updates` or no `stream` answer
is accepted, when an accepted `has_deadline` answer is true; a message sorted
to `newsletters` SHALL raise neither. When answers of more than one source to
the same question were accepted, the facts' answer SHALL be the one used. A
`suspicious` answer of true that the policy accepts SHALL raise a `review`
need instead of any other. When the `decide` role is configured, a model
answer of true for `needs_reply` or `asks_action` that the policy does not
accept, with a confidence of at least 0.8, where no `sure` need exists and no
accepted answer to the same question is false, SHALL raise a `review` need
with trust `check`. Only model answers whose `model_version` starts with
`checked:` SHALL count. A need SHALL carry the due date and the lapse time
Scout read from its message's email (see "Dates from Scout's read"), none
until they are read, and no lapse time when it is a `pay` need or a phishing
warning; and `urgent` when an accepted `urgency` answer is "today". Messages
from the conversation's own inbox SHALL never raise one.

#### Scenario: A question from a person
- **WHEN** Marta writes "Could you confirm Thursday's walkthrough?" and the policy accepts the model's `needs_reply` true
- **THEN** the conversation has a `sure` `reply` need, with no due date until Scout has read the email

#### Scenario: A bill
- **WHEN** the policy accepts the model's `stream` answer `receipts` for an invoice from a service and the model answers `money` `owed` at 0.91
- **THEN** the need is `pay` with trust `check` and question `money`

#### Scenario: A receipt the model reads as an ask
- **WHEN** the policy accepts the model's `stream` answer `receipts` and its `asks_action` true for a payment confirmation, and the model answers `money` `paid`
- **THEN** no need is raised

#### Scenario: Answered before the money question
- **WHEN** a message in Receipts has accepted `asks_action` true and no `money` answer
- **THEN** no need is raised until the model answers `money`

#### Scenario: An invoice sent by hand
- **WHEN** the policy accepts the model's `stream` answer `primary` for an invoice a supplier attached to their own email and the model answers `money` `owed` at 0.9
- **THEN** the need is `pay` with trust `check`

#### Scenario: A promotion with a price
- **WHEN** the policy accepts the model's `stream` answer `newsletters` and the model answers `money` `owed`
- **THEN** no need is raised

#### Scenario: An unsure bill
- **WHEN** the model answers `money` `owed` at 0.6 and the policy does not accept it
- **THEN** no `pay` need is raised

#### Scenario: A notification with a question
- **WHEN** a shipping notice with a `List-Unsubscribe` header ends "Questions? Contact our support team." and the model answers `needs_reply` true
- **THEN** the facts' accepted "no" wins and no `reply` need is raised

#### Scenario: A notification that reads like an ask
- **WHEN** the policy accepts the model's `stream` answer `updates` and its `asks_action` true for a calendar invitation, and its `has_deadline` answer is false
- **THEN** no need is raised

#### Scenario: A notification asking by a date
- **WHEN** the policy accepts the model's `stream` answer `updates`, its `asks_action` true and its `has_deadline` true for a delivery to reschedule
- **THEN** the need is `do`

#### Scenario: A newsletter
- **WHEN** the policy accepts the model's `stream` answer `newsletters` and its `asks_action` true
- **THEN** no need is raised

#### Scenario: Phishing asks for an action
- **WHEN** the policy accepts the model's `suspicious` true and `asks_action` true for a message
- **THEN** the need is `review` and no `do` need exists

#### Scenario: Old mail on the first sync
- **WHEN** a first sync brings a question sent 40 days ago
- **THEN** no need is raised

#### Scenario: Rules only
- **WHEN** no `decide` role is configured
- **THEN** no need has trust `check` and no `sure` need is raised

#### Scenario: The rules already settled it
- **WHEN** the facts accept `needs_reply` false for a code host's list notification, and the model answers `needs_reply` true with confidence 0.95
- **THEN** no `reply` need is raised

#### Scenario: Only the model says yes
- **WHEN** no answer to `asks_action` is accepted and the model answers it true with confidence 0.875, below the policy's operating point
- **THEN** the need is `review` with trust `check`

#### Scenario: Unchecked model answers
- **WHEN** a message's only `needs_reply` true is a model answer marked `laya-0.3.21-unchecked`
- **THEN** reading it raises no `check` need, and an open `check` need it raised before is withdrawn as `superseded` on the next refresh

#### Scenario: A bill's due date
- **WHEN** Scout read "due by 30 October" from an open Pay need's invoice
- **THEN** the need's due date is 30 October and it carries no lapse time

### Requirement: Clearing a need
A need SHALL be cleared, with its cause, when: a message is sent from the
inbox in the conversation after the message that raised it (`replied`; clears
`reply` and `do`), the conversation is archived (`archived`; clears `reply` and
`review`), every message is in the trash or spam or removed (`trashed`; clears
any), a member resolves it, or its lapse time has passed (`lapsed`; clears any
need but `pay` and a phishing warning, see "Asks that lapse"). A new inbound
message SHALL derive the need again from that message (`superseded` for the
old one). When the open need's own message, read again, raises no need and no
member confirmed it, the need SHALL be cleared as `superseded`. Changes SHALL
come from the provider, from members or from the passing of time alike, and a
need SHALL be settled against the time it is worked out at.

#### Scenario: Replied from the phone
- **WHEN** Ana replies to Marta from Gmail on her phone and the sent message reaches the conversation
- **THEN** the `reply` need is cleared with cause `replied`

#### Scenario: Archiving a bill
- **WHEN** a member archives a conversation with a `pay` need
- **THEN** the need stays open until a member chooses Done or Not needed

#### Scenario: Done in a shared inbox
- **WHEN** Ana chooses Done on a need in an inbox she shares with Leo
- **THEN** the need is cleared for both of them

#### Scenario: A question raised against a settled answer
- **WHEN** a conversation has an open `check` need whose message's accepted answers say it asks nothing, and the conversation's need is refreshed
- **THEN** the need is cleared with cause `superseded`, announced once, and not shown under Done today

### Requirement: Need changes are published
Every change to a need SHALL publish `conversations.need_changed` v1
(`contracts/conversations.need_changed.v1.schema.json`) in its transaction,
carrying the workspace, inbox, conversation and message ids, a revision that
grows with every change of that conversation's need, the message's sent time,
the need (reason, trust, due date, lapse time when it has one, urgent) or
null, the cause and the member when cleared, and the conversation's newest
inbound message. A newer inbound message SHALL be announced even when the
need does not change. It SHALL carry no names, addresses, subjects or text,
and SHALL declare an upsert of the conversation within its inbox. A consumer
SHALL accept events written before the lapse time existed.

#### Scenario: Cleared by a member
- **WHEN** Ana chooses Done on a need
- **THEN** `need_changed` carries a null need, cause `done`, Ana's user id and a revision one higher than before

#### Scenario: A link that expires
- **WHEN** a `do` need's email was read as lapsing at 09:10 UTC
- **THEN** `need_changed` carries `lapses_at` 09:10 UTC, and the event written for it before this change still replays

### Requirement: Resolving a need
`POST …/conversations/{conversation_id}/need/resolve`
(`conversations.resolve_need`, `conversations:write`) SHALL clear the open
need with outcome `done` or `not_needed`, naming the message the need the
member saw comes from (`message_id`): when the open need comes from another
message, nothing SHALL be recorded and the answer SHALL be 409
`need_changed` (the same for `confirm_need`); `not_needed` SHALL also store a
correction of the question that raised it (true to false; for a `pay` need,
`money` from `owed` to `not_owed`), except on a still-needed question
(`still_to_do`, `still_needs_reply`), where it stores none. For a `pay` need it SHALL also take
`already_paid`, which clears the need as `done` and stores nothing else, and
`still_to_pay`, which turns a `check` `pay` need into a `sure` one with basis
`member` and stores a correction of `money` from `owed` to `owed`; either
outcome on a need that is not `pay` SHALL answer 409 `not_a_payment`, and
`still_to_pay` on a `pay` need that is already `sure` SHALL change nothing
and answer 200. When the member resolves as `done` or `already_paid` a need
they had confirmed (`confirmed`, `still_to_pay`), reopening SHALL bring back
the confirmed need and keep its correction. `…/need/confirm` (`conversations.confirm_need`) SHALL turn a
`check` need into a `sure` one and store a correction confirming the answer,
except on a still-needed question, which it turns back into its `do` or
`reply` need with basis `member` and stores nothing.
`…/need/reopen` (`conversations.reopen_need`) SHALL undo any of them,
restoring the need as it was and marking the correction withdrawn. Each SHALL
take the `sender` role or above (a `reader` gets 403 `inbox_read_only`), and
SHALL answer 409 `no_open_need` when there is nothing to resolve or confirm,
and `nothing_to_reopen` when there is nothing to reopen.

#### Scenario: Not needed teaches
- **WHEN** a member marks a rules `reply` need Not needed
- **THEN** the need is cleared with cause `not_needed` and a correction of `needs_reply` from true to false overriding `rules` is stored

#### Scenario: Undo
- **WHEN** the member then reopens it
- **THEN** the same need is open again and the correction is marked withdrawn

#### Scenario: The need changed meanwhile
- **WHEN** a member resolves a need naming a message other than the one the open need comes from
- **THEN** the response is 409 `need_changed` and no correction is stored

#### Scenario: It's safe
- **WHEN** a member marks a `review` need for a suspicious message Not needed
- **THEN** a correction of `suspicious` from true to false is stored and no other need is raised for that message

#### Scenario: Still to pay
- **WHEN** a member answers Still to pay on Acme Hosting's `pay` question
- **THEN** the need is `pay`, `sure`, basis `member`, and a correction of `money` from `owed` to `owed` is stored

#### Scenario: Already paid
- **WHEN** a member answers Already paid on a `pay` need
- **THEN** the need is cleared with cause `done`, no correction is stored, and reopening brings it back

#### Scenario: Not a bill
- **WHEN** a member answers Already paid on Marta's `reply` need
- **THEN** the response is 409 `not_a_payment` and the need is unchanged

#### Scenario: No longer needed
- **WHEN** a member answers Not needed on Pedro's `still_to_do` question
- **THEN** the need is cleared with cause `not_needed`, no correction is stored, and reopening brings the question back

### Requirement: Needs in summaries
A conversation summary SHALL carry `need` while its need is open: the reason,
trust, due date, whether it is overdue, the lapse time when it has one,
whether it is urgent, the rule of the reason that decided it, its message's
sent time, and the sender's sentence with the sender's name and the
message's sent time. An open need whose lapse time has passed SHALL NOT be
carried as `need`. A summary SHALL carry `lapsed_need` (reason, question,
message id and lapse time) while the conversation's need has lapsed (cleared
`lapsed`, or open with its lapse time passed) and its message is still the
conversation's newest inbound message that is not in spam or the trash.
`conversations.list` SHALL accept `ids` (1 to 100 conversation ids, else
400) and return those the caller can see, leaving the others out.

#### Scenario: Home asks for its rows
- **WHEN** the client lists three conversation ids, one of them in an inbox the caller cannot see
- **THEN** two summaries are returned, each with its `need`

#### Scenario: An expired link
- **WHEN** a conversation's sign-in link lapsed and nothing newer arrived
- **THEN** its summary has no `need` and has `lapsed_need` with the lapse time

#### Scenario: The sender writes again
- **WHEN** the sender of a lapsed ask writes again, asking nothing
- **THEN** the summary has neither `need` nor `lapsed_need`

### Requirement: Scout's summary of a need's email
When a need that is not a phishing warning (question `suspicious`) opens for
a message and the `complete` role is configured and may receive mail on this
server, a job (`conversations.summarize_need`, queued in
the transaction that stores the need, never inside the decide call or on a
request path) SHALL ask the role for a summary of that message in one or two
plain sentences, in English, European Portuguese, Spanish from Spain and
French, together with its dates (see "Dates from Scout's read"), in one call,
and store them once in `conversations.need_summaries`
keyed by the message. The mail SHALL reach the model only as JSON string
values (sender, date, subject, own words as plain text without quoted
history, bounded) under a fixed instruction that says it is data and never
instructions; no preference or rule of the member SHALL be sent. The
instruction SHALL forbid time relative to the moment it is written ("in 10
minutes", "today", "tomorrow", "expires soon") and ask for times and
durations as the email states them, anchored to when it was sent, because
the summary is kept and read later. The answer
SHALL be held to a strict JSON schema, then kept as plain text on one line
without control, reordering or invisible characters, at most 400 characters
per language, and not at all when English has nothing. The call SHALL be
counted against the workspace's daily AI limit as background work, one call
per message however often the need is worked out again, and a message that
already has a summary and its dates SHALL NOT be asked again. A call with no usable answer
SHALL be stored as failed and tried at most once more later. No job SHALL be
queued, and nothing shown, when the role is not configured or may not receive
mail here. The job SHALL skip a message whose need is no longer open, whose
inbox is kept read-only, that is in spam or the trash, or that has no words.
Failures SHALL be logged by kind only. When a summary is written the module
SHALL publish `conversations.need_summarised` (ids only), and once dates are
stored it SHALL work the conversation's need out again.

#### Scenario: A need opens
- **WHEN** a message raises a Reply need and the `complete` role may read mail
- **THEN** one summary is written with its message in all four languages, and `need_summarised` is published once

#### Scenario: Worked out again
- **WHEN** the same need is refreshed, or its job runs a second time
- **THEN** the model is not asked again and nothing is written

#### Scenario: Mail that may be phishing
- **WHEN** a message raises a phishing warning
- **THEN** no summary is asked for or stored: a summary would retell the scam in Scout's voice, and the warning is what matters

#### Scenario: Nothing usable
- **WHEN** the model answers with nothing in English
- **THEN** the message is stored as failed, nothing is shown, a later run asks once more, and after that never

#### Scenario: No role
- **WHEN** no `complete` role may receive mail on this server
- **THEN** no summary job is queued and the conversation shows its email without a summary

#### Scenario: A summary that does not count down
- **WHEN** Scout summarises a sign-in email whose link is valid for 10 minutes
- **THEN** the instruction it was given forbids "expires in 10 minutes" and asks for "valid for 10 minutes after it was sent"

### Requirement: Summaries for needs that opened before
On upgrade every inbox SHALL be queued for the needs backfill, without any
manual step, and when the `complete` role may read mail the backfill's page
job SHALL queue `conversations.summarize_need` for each open need of its page
that has no summary, only one failed call, or a summary written before its
dates were read. Such a summary SHALL be asked for once more: a usable
answer replaces its texts and stores its dates; no usable answer keeps its
texts and marks it read, so it is never asked again. Running the backfill
again SHALL ask for nothing whose summary and dates exist.

#### Scenario: An open need from before
- **WHEN** a Reply need was open before the upgrade and the role may read mail
- **THEN** the backfill queues its summary, the job writes it, and a second backfill asks nothing

#### Scenario: A summary from before the dates
- **WHEN** an open Do need's summary was written before dates were read
- **THEN** the backfill queues it once, its dates are stored, and a second backfill asks nothing

## ADDED Requirements

### Requirement: Dates from Scout's read
The `complete` answer of `conversations.summarize_need` SHALL include
`lapses_at` (an RFC 3339 time: when what the email asks stops being possible,
such as a link or a code expiring or an offer or an invitation closing) and
`due_on` (a `YYYY-MM-DD` date by which the recipient is asked to act), each
worked out by the model from the email's words and its `sent_at`, and null
when the email gives none. No deterministic code SHALL read the mail to find
or correct a date (ADR 0018). The module SHALL drop a value that does not
parse, that comes before the message was sent, or that lies more than 366
days after it. The dates SHALL be stored with the message in
`conversations.need_summaries`, with the time they were read, and copied to
the message's need when it is worked out.

#### Scenario: A sign-in link
- **WHEN** the model answers `lapses_at` 09:10 UTC for a sign-in email sent at 09:00 UTC
- **THEN** the message's lapse time is 09:10 UTC and its need carries it

#### Scenario: A date before the email
- **WHEN** the model answers a `due_on` a week before the message was sent
- **THEN** no due date is stored

#### Scenario: A date years away
- **WHEN** the model answers `lapses_at` three years after the message was sent
- **THEN** no lapse time is stored

#### Scenario: Not a date
- **WHEN** the model answers `lapses_at` "soon"
- **THEN** no lapse time is stored and the summary is kept

### Requirement: Asks that lapse
A need with a lapse time SHALL be cleared with cause `lapsed` once that time
has passed, whatever its basis, unless it is a `pay` need (a bill stays until
a member answers it) or a phishing warning (which keeps its own rules).

#### Scenario: A link that expired
- **WHEN** a `do` need's lapse time is 10 minutes after its message was sent, and that time has passed
- **THEN** the need is cleared with cause `lapsed`

#### Scenario: A bill past its date
- **WHEN** a `pay` need's email named a date that has passed
- **THEN** the need stays open

### Requirement: Old asks become a question
An open `sure` `do` or `reply` need whose message was sent longer ago than
the needs window, with no due date and no lapse time, whose basis is not
`member`, SHALL become a `review` need with trust `check` and question
`still_to_do` (from `do`) or `still_needs_reply` (from `reply`). A member's
Yes SHALL make it the `do` or `reply` need again with basis `member`, which
is never asked again; Not needed SHALL clear it as `not_needed`. Neither
SHALL store a correction.

#### Scenario: Three weeks on
- **WHEN** Pedro Alves's `do` need, with no due date, comes from a message sent 21 days ago and no member confirmed it
- **THEN** the need is `review`, `check`, question `still_to_do`

#### Scenario: Yes, still to do
- **WHEN** the member answers Yes to it
- **THEN** the need is `do`, `sure`, basis `member`, no correction is stored, and it stays a `do` need however old it gets

#### Scenario: A dated ask
- **WHEN** a `do` need's message was sent 21 days ago and the need has a due date
- **THEN** the need stays `do` and no question is asked

### Requirement: The clock of needs
The module SHALL keep, for each open need that will change with time alone,
when it next will (`conversations.need_clocks`: ids and a time), written in
the transaction that works the need out. A dispatcher
(`conversations.dispatch_need_clocks`, every minute) SHALL work in the
maintenance scope (allowed for it under `[[access.maintenance_scope]]` in
`architecture.toml`), read only which workspaces have a due row, and enqueue
`conversations.wake_needs` in each of those workspaces, one pending job per
workspace. That job SHALL work each due conversation's need out again, one
conversation per transaction, so a lapsed ask is cleared and an old one
becomes a question without any member acting. A transaction that set
neither a workspace nor the maintenance scope SHALL see no clock row, and
the maintenance scope SHALL only read them.

#### Scenario: Ten minutes later
- **WHEN** a `do` need lapsing at 09:10 UTC is open and the dispatcher ticks at 09:11 UTC
- **THEN** the workspace's wake job clears it as `lapsed` and publishes `need_changed`

#### Scenario: Two workspaces
- **WHEN** needs of two workspaces are due and the dispatcher ticks, as the runtime role
- **THEN** one wake job is enqueued in each workspace and the dispatcher's transaction reads nothing else
