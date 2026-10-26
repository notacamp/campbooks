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
says it needs a reply or an action. Otherwise a `sure` need SHALL come only from answers Campbooks stands behind: `reply` when `needs_reply` is true, else `do` when `asks_action` is true. The facts' and a taught rule's answer stands behind a need where the policy accepts it. The model's answer stands behind one only with a confidence of at least the sure bar of its question (`Policy::sure_bar`: 0.9 for `needs_reply`, and then only when no `suspicious` answer of the message is true; 0.8 for `asks_action`), and only when the inbox is in `To` of the message: for a message the inbox was only copied on, or not named in, it stands behind one only when the policy accepted `next_move` `recipient`. A `reply` or `do`
need, `sure` or `check`, SHALL be raised only when the message's accepted
`stream` answer is `primary`, or, when it is `updates` or no `stream` answer
is accepted, when an accepted `has_deadline` answer is true; a message sorted
to `newsletters` SHALL raise neither. When answers of more than one source to
the same question were accepted, the facts' answer SHALL be the one used. A
`suspicious` answer of true that the policy accepts SHALL raise a `review`
need instead of any other. When the `decide` role is configured, a model answer of true for `needs_reply` or `asks_action` with a confidence of at least 0.8 that does not stand behind a need, where no `sure` need exists and no accepted answer to the same question is false, SHALL raise a `review` need with trust `check`, a question to the member. A model answer of true below 0.8 SHALL raise nothing, whether the policy accepts it or not. Only model answers whose `model_version` starts with
`checked:` SHALL count. A need SHALL carry the due date and the lapse time
Scout read from its message's email (see "Dates from Scout's read"), none
until they are read, and no lapse time when it is a `pay` need or a phishing
warning; and `urgent` when an accepted `urgency` answer is "today". Messages
from the conversation's own inbox SHALL never raise one.

#### Scenario: A question from a person
- **WHEN** Marta writes "Could you confirm Thursday's walkthrough?" to the inbox and the model answers `needs_reply` true at 0.95
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
- **WHEN** no answer to `asks_action` is accepted, the model answers it true with confidence 0.875 and the inbox is only copied on the message
- **THEN** the need is `review` with trust `check`

#### Scenario: A guess
- **WHEN** the model answers `needs_reply` true at 0.59 for a message from a colleague to a third party that copies the inbox
- **THEN** no need is raised, though the policy acts on `needs_reply` from 0.51

#### Scenario: Between the floor and the bar
- **WHEN** the inbox is in `To` and the model answers `needs_reply` true at 0.85
- **THEN** the need is `review` with trust `check` and question `needs_reply`

#### Scenario: A look-alike
- **WHEN** the model answers `needs_reply` true at 0.95 and `suspicious` true at 0.52, below the policy's operating point
- **THEN** the need is a `check` question, not a `sure` reply

#### Scenario: Only copied
- **WHEN** the inbox is in `Cc` of a message and the model answers `needs_reply` true at 0.97
- **THEN** the need is a `check` question, and a rule the member taught that says it needs a reply still raises a `sure` `reply`

#### Scenario: An action from 0.8
- **WHEN** the inbox is in `To` and the model answers `asks_action` true at 0.8, and at 0.79
- **THEN** the first raises a `sure` `do` need and the second raises nothing

#### Scenario: Unchecked model answers
- **WHEN** a message's only `needs_reply` true is a model answer marked `laya-0.3.21-unchecked`
- **THEN** reading it raises no `check` need, and an open `check` need it raised before is withdrawn as `superseded` on the next refresh

#### Scenario: A bill's due date
- **WHEN** Scout read "due by 30 October" from an open Pay need's invoice
- **THEN** the need's due date is 30 October and it carries no lapse time

### Requirement: The decide pass records suggestions
Every answer of every strategy SHALL be stored in `decisions` with its
question, source, value, options, probabilities, confidence, model, model
version, latency, reasons, whether it acted, and the envelope version
(`1 + CONTEXT_VERSION` of the triage context). The rules SHALL be the
shared facts strategy and the questions the shared triage questions of
`campbooks_ai::decide`; this module SHALL hold no rules of its own. The model pass
(`conversations.decide_message`, role `decide`) SHALL ask the nine questions (`stream`, `needs_reply`, `asks_action`, `has_deadline`, `urgency`, `suspicious`, `money`, `has_dated_thing`, `dated_kind`) and, only while whose move it is is switched on (`CAMPBOOKS_CONVERSATIONS__MOVES__ENABLED`, off by default, apart from the action catalogue's switch), `next_move`, and `awaited` when the facts leave someone who may be awaited, in one call per
inbound message, telling the model whose inbox the message is in and the
facts as hints, only when the `decide` role is configured and has passed its
answers check, only for messages sent within
`CAMPBOOKS_CONVERSATIONS__DECIDE_BACKFILL_DAYS` (default 14), and once per
message. No other `decide` call SHALL be made about a message. When the role
has not passed its answers check, the pass SHALL do exactly what it does with
no `decide` role: store nothing, fail no job. Answers the policy accepts
SHALL be marked `acted`; the shipped policy (`Policy::baseline`) SHALL let a
model answer act from the operating point where the committed eval report
shows the model run holding its question's target, and a question the report
does not measure (`money`, `next_move`, `awaited`, `has_dated_thing`,
`dated_kind`) SHALL only be recorded. An answer to `next_move`, `awaited` or
`dated_kind` that is missing or outside its options SHALL record nothing and
fail nothing. Provider failures SHALL be retried, recording only the kind of
failure.

#### Scenario: A confident but wrong model answer
- **WHEN** the policy accepts model `needs_reply` answers from 0.9 and the model answers true at 0.7
- **THEN** the answer is stored with `acted = false`

#### Scenario: Old mail
- **WHEN** a batch brings mail sent before the backfill window
- **THEN** the model is not called

#### Scenario: A replayed delivery
- **WHEN** `message_received` is delivered again to `conversations.decide_message`
- **THEN** the model is not asked again

#### Scenario: A model that failed its answers check
- **WHEN** the decide role's last answers check failed and a batch of mail arrives
- **THEN** the model is not called, and the decisions, streams and needs are the same as with no `decide` role

#### Scenario: The recipient
- **WHEN** a message arrives in Sam's inbox, which only Sam reads
- **THEN** the model reads "The recipient is sam@example.com, who reads this personal inbox."

#### Scenario: The money answer is recorded
- **WHEN** the model answers `money` `owed` at 0.95 for a bill
- **THEN** the answer is stored with `acted = false`, since the committed report does not measure `money`

#### Scenario: Who is awaited, by the headers
- **WHEN** João writes to Ana Costa, copying the inbox, and the model answers `awaited` `p1`
- **THEN** the stored answer keeps its options, and `p1` names Ana Costa's address as the headers gave it

#### Scenario: A booking in the same call
- **WHEN** a hotel's booking confirmation arrives and the model answers `has_dated_thing` true at 0.93 and `dated_kind` `stay`
- **THEN** both answers are stored with `acted = false` from the one call, and no second call is made

### Requirement: Waiting on someone
When the newest inbound message raises no other need (`worth_a_look`, a rule
the member taught, included), its accepted `stream`
answer is `primary` or no `stream` answer is accepted, and no fact marks it as
list, automatic or no-reply mail, the conversation SHALL get a `waiting`
need on one person: the sender, when the policy accepts `next_move` `sender`
or the model answers it with a confidence of at least 0.8; or the person a
model `awaited` answer names (not `none`), when the policy accepts
`next_move` `someone_else` or the model answers it with a confidence of at
least 0.8. The need SHALL hold that person's name and address as the headers
gave them, trust `sure` when the policy accepted `next_move` and `check`
otherwise, and a nudge time: when Scout's read of the email gives a `due_on`
date (a read happens only for a flagged message), the working day after it,
else the message's sent time plus
`CAMPBOOKS_CONVERSATIONS__WAIT__WORKING_DAYS` (default 3; 1 to 15, else the
server refuses to start) working days, working days being Monday to Friday by
UTC date, at the time of day the message was sent. Its due date SHALL be the
nudge time's date. The need clock SHALL wake it at the nudge time and turn it
into a `nudge` need, with a new revision. Only model answers whose
`model_version` starts with `checked:` SHALL count. No wait SHALL be on one
of the inbox's own addresses (mail the inbox sent itself, which Gmail and
Zoho file as received). While whose move it is is switched off (`CAMPBOOKS_CONVERSATIONS__MOVES__ENABLED`, apart from the action catalogue's switch), no new wait SHALL be raised; a wait already open SHALL keep its course. A wait the member asked for (see "Someone else has the move") SHALL be raised whatever the switch says.

#### Scenario: Ana was asked to write
- **WHEN** João's message of Wednesday 14 October 10:00 asks Ana Costa to send the floor plan, raises no other need, and the model answers `next_move` `someone_else` at 0.91 and `awaited` Ana Costa
- **THEN** the conversation has a `check` `waiting` need on Ana Costa with the nudge time Monday 19 October 10:00

#### Scenario: A promise with a date
- **WHEN** Rui writes "I'll send the signed contract by Friday 16 October", the pass flags it with `has_deadline`, the model answers `next_move` `sender` at 0.86, and Scout's read gives `due_on` 16 October
- **THEN** the need waits on Rui with the nudge time Monday 19 October, at the time Rui's message was sent

#### Scenario: A promise nobody flagged
- **WHEN** Rui's promise is not flagged, so it is not read
- **THEN** the need waits on Rui with the nudge time three working days after his message

#### Scenario: Unsure
- **WHEN** the model answers `next_move` `someone_else` at 0.6
- **THEN** no `waiting` need is raised

#### Scenario: Someone else, but nobody named
- **WHEN** the model answers `next_move` `someone_else` at 0.9 and `awaited` `none`
- **THEN** no `waiting` need is raised

#### Scenario: The member's move wins
- **WHEN** the message also raises a `reply` need
- **THEN** the need is `reply` and nothing waits

#### Scenario: The clock
- **WHEN** Monday 19 October 10:00 passes and Ana has not written
- **THEN** the wake job turns the need into `nudge` and publishes `need_changed`

### Requirement: Waits for mail already here
When whose move it is is switched on (`CAMPBOOKS_CONVERSATIONS__MOVES__ENABLED`), every inbox whose needs backfill finished without it SHALL be queued once more (`needs_backfills.moves_asked`) to ask `next_move` and `awaited` about its inbound messages within the needs window whose
conversation holds no open need and whose other answers are on record, within
the workspace's daily AI limit; the needs SHALL then be worked out again.
Messages SHALL not be asked their other questions again.

#### Scenario: Switched on later
- **WHEN** the switch is turned on after the upgrade, and the needs backfill of every inbox had finished
- **THEN** each inbox is queued once and the messages of the last 14 days that raised no need are asked, and with the switch off nothing is queued

#### Scenario: João's email from last week
- **WHEN** the upgrade runs and João's message of 14 October, which raised no need, has no `next_move` answer
- **THEN** it is asked `next_move` and `awaited` once, and the conversation waits on Ana from the answers

## ADDED Requirements

### Requirement: The people behind a question
A need that is a question about a reply or an action (`review`, trust
`check`, question `needs_reply` or `asks_action`) on a message that went to
people besides the inbox SHALL carry `unsure` in its summary: those people
(`To` before `Cc`, as the headers gave them, without the inbox, the sender and
no-reply or service addresses, at most six), whether the inbox is in `To` and whether it is in `Cc`, the
one natural party (the person the model's `awaited` answer named, when its
stored option still names that address, else the only other person in `To`;
none when neither exists), the days since the message when none of those
people wrote to the inbox since (none when one did), and the model's
confidence and the sure bar of the question in whole percent. It SHALL be
worked out from the headers, the stored answers and the inbox's messages
only, never from what a message says. A message that went to nobody else
SHALL carry none.

#### Scenario: Copied on mail between others
- **WHEN** João writes to Ana Costa, copying the inbox, and the model answers `needs_reply` true at 0.97
- **THEN** the need is a `check` question whose `unsure` lists Ana Costa, says the inbox is not in `To`, names her as the party, and carries confidence 97 and sure bar 90

#### Scenario: Written to the inbox alone
- **WHEN** Marta writes to the inbox alone and the model answers `needs_reply` true at 0.85
- **THEN** the need is a `check` question with no `unsure`

#### Scenario: She wrote back
- **WHEN** Ana Costa wrote to the inbox after João's message
- **THEN** `unsure` carries no days unanswered

### Requirement: Someone else has the move
`POST /api/v1/w/{workspace_id}/conversations/{conversation_id}/need/resolve`
(`conversations.resolve_need`) SHALL take an optional `wait_on`, the address of
a person, together with the outcome `not_needed`, for a need that is a
question about a reply or an action. The address SHALL be compared without
regard to case with the people the facts leave awaitable for the need's
message, else the request SHALL get 409 `not_a_party` and nothing SHALL be
stored; with another outcome, or for another need, it SHALL get 409
`no_open_need`. It SHALL file two corrections in the transaction: of
`next_move`, from the model's stored answer (`recipient` when none) to
`someone_else`, and of `awaited`, from the stored answer (`none` when none) to
the option that names that person, each citing the answers it corrects. The
verdict SHALL be stored with that person and the second correction, and the
need SHALL then be a `waiting` need on that person with trust `sure`, basis
`member` and question `next_move`, its nudge time worked out as for any wait,
whatever the switch of whose move it is says. It SHALL stay that while its
message is the newest; a newer message replaces it like any need. Reopening
(`conversations.reopen_need`) SHALL withdraw both corrections, clear the person
and bring the question back.

#### Scenario: Nudge Ana
- **WHEN** Sam answers João's question "Not needed" with `wait_on` ana@studio.example
- **THEN** the conversation waits on Ana Costa, a correction of `next_move` from `recipient` to `someone_else` and one of `awaited` from `none` to `p1` are filed, and Undo withdraws both and brings the question back

#### Scenario: Not on the email
- **WHEN** the address is eve@elsewhere.example
- **THEN** the request gets 409 `not_a_party` and no correction is filed

#### Scenario: Only Not needed
- **WHEN** the outcome is `done` with a `wait_on`
- **THEN** the request gets 409 `no_open_need`

### Requirement: Needs worked out again under a new policy
The migration that changed the policy SHALL give every open need a clock due
now, so that `conversations.wake_needs` works each one out again from the
stored answers: no model SHALL be asked and no rule SHALL run again. A `sure`
`reply` or `do` need the policy does not stand behind SHALL be announced as a
`review` question with trust `check`; one whose answer is below the check
floor, with no confirmation of a member behind it, SHALL be cleared as
`superseded`; a need a member confirmed SHALL keep its place. Running it again
SHALL change nothing.

#### Scenario: The 0.59 reply
- **WHEN** an open `sure` `reply` need rests on a model `needs_reply` true at 0.59 and the upgrade runs
- **THEN** the need is cleared as `superseded` and `need_changed` says so

#### Scenario: A reply at 0.85
- **WHEN** an open `sure` `reply` need rests on `needs_reply` true at 0.85
- **THEN** it becomes a `check` question and the member can answer it

#### Scenario: Confirmed already
- **WHEN** a member said Yes to the question earlier
- **THEN** the upgrade leaves the need as it is
