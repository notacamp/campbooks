## MODIFIED Requirements

### Requirement: Streams from accepted answers
Every conversation SHALL be in exactly one stream: `primary`, `updates`,
`receipts` or `newsletters`. A new conversation's stream SHALL be set from
its first inbound message, only from an answer to the `stream` question that
the policy accepts; otherwise it SHALL be `primary`. The facts never answer
`stream`, so a conversation is stored in `primary` and the model pass places
it: when the policy accepts the model's `stream` answer for the
conversation's first inbound message and the conversation's stream source is
still `default`, the conversation SHALL move to that stream with stream
source `model`. Later messages SHALL never re-sort a conversation, and a
member's move SHALL stick.

#### Scenario: No rules strategy
- **WHEN** the module runs with no `decide` role
- **THEN** every conversation is in `primary` with stream source `default`

#### Scenario: The shipped policy
- **WHEN** the facts strategy runs on a message with mailing-list headers
- **THEN** it gives no `stream` answer and the conversation is stored in `primary`

#### Scenario: A policy that lets rules place the stream
- **WHEN** a test policy accepts rules stream answers from 0.8 and a test rules strategy answers `newsletters` at 0.95
- **THEN** the conversation is in `newsletters` with stream source `rules`, and the answer is stored as acted on with its reasons

#### Scenario: The model sorts a newsletter
- **WHEN** the policy accepts model stream answers from 0.9 and the model answers `newsletters` at 0.98 for a conversation's first inbound message
- **THEN** the conversation is in `newsletters` with stream source `model`, and the answer is stored as acted

#### Scenario: A member's move sticks
- **WHEN** a member moved a conversation to `primary` before the model pass answers `newsletters` with an accepted confidence
- **THEN** the conversation stays in `primary` and the answer is stored with `acted = false`

### Requirement: The decide pass records suggestions
Every answer of every strategy SHALL be stored in `decisions` with its
question, source, value, options, probabilities, confidence, model, model
version, latency, reasons, whether it acted, and the envelope version
(`1 + CONTEXT_VERSION` of the triage context). The rules SHALL be the
shared facts strategy and the questions the shared triage questions of
`campbooks_ai::decide`; this module SHALL hold no rules of its own. The model pass
(`conversations.decide_message`, role `decide`) SHALL ask the six questions
(`stream`, `needs_reply`, `asks_action`, `has_deadline`, `urgency`,
`suspicious`) in one call per inbound message, telling the model whose inbox
the message is in and the facts as hints, only when the `decide` role is
configured and has passed its answers check, only for messages sent within
`CAMPBOOKS_CONVERSATIONS__DECIDE_BACKFILL_DAYS` (default 14), and once per
message. When the role has not passed its answers check, the pass SHALL do
exactly what it does with no `decide` role: store nothing, fail no job.
Answers the policy accepts SHALL be marked `acted`; the shipped policy
(`Policy::baseline`) SHALL let a model answer act from the operating point
where the committed eval report shows the model run holding its question's
target. Provider failures SHALL be retried, recording only the kind of
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

### Requirement: The need of a conversation
The module SHALL keep at most one open need per conversation, derived from the
newest inbound message sent within
`CAMPBOOKS_CONVERSATIONS__NEEDS_WINDOW_DAYS` (default 14), its decisions, the
policy and the conversation's state. A `sure` need SHALL come only from
answers the policy accepts: `pay` when `asks_action` is accepted and true and
the message's accepted `stream` answer is `receipts`, else `reply` when
`needs_reply` is accepted and true, else `do` when `asks_action` is accepted
and true. A `reply` or `do` need, `sure` or `check`, SHALL be raised only
when the message's accepted `stream` answer is `primary`, or, when it is
`updates` or no `stream` answer is accepted, when an accepted `has_deadline`
answer is true; a message sorted to `newsletters` SHALL raise neither. When
answers of more than one source to the same question were
accepted, the facts' answer SHALL be the one used. A `suspicious` answer of
true that the policy accepts SHALL raise a `review` need instead of any
other. When the `decide` role is configured, a model answer of true for
`needs_reply` or `asks_action` that the policy does not accept, with a
confidence of at least 0.8, where no `sure` need exists and no accepted
answer to the same question is false, SHALL raise a `review` need with trust
`check`. Only model answers whose `model_version` starts with `checked:`
SHALL count. A need SHALL carry no due date, and `urgent` when an accepted
`urgency` answer is "today". Messages from the conversation's own inbox SHALL
never raise one.

#### Scenario: A question from a person
- **WHEN** Marta writes "Could you confirm Thursday's walkthrough?" and the policy accepts the model's `needs_reply` true
- **THEN** the conversation has a `sure` `reply` need with no due date

#### Scenario: A bill
- **WHEN** the policy accepts the model's `stream` answer `receipts` and its `asks_action` true for an invoice from a service
- **THEN** the need is `pay`

#### Scenario: An invoice sent by hand
- **WHEN** the policy accepts the model's `stream` answer `primary` and its `asks_action` true for an invoice a supplier attached to their own email
- **THEN** the need is `do`

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

## ADDED Requirements

### Requirement: Upgrading from content rules
On upgrade, every stored rules answer with an envelope version below 3 SHALL
be marked not acted (kept as a record), and every inbox SHALL be queued for
the needs backfill, so a need raised only by those answers is withdrawn as
`superseded`. When a trusted `decide` role is configured, the backfill SHALL
ask the model about each conversation's newest inbound message sent within
the decide backfill window that has no checked model answer, once per
message.

#### Scenario: A need the old rules raised
- **WHEN** a shipping notice had an open `reply` need raised by the old `signals.question` rule and the upgrade runs
- **THEN** the need is withdrawn as `superseded` and Home drops it

#### Scenario: Recent mail is read again
- **WHEN** the upgrade runs with Jev configured and a conversation's newest inbound message from three days ago has no checked model answer
- **THEN** the model is asked about it once and its accepted answers sort the conversation and raise its need
