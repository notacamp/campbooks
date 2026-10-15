## MODIFIED Requirements

### Requirement: The decide pass records suggestions
Every answer of every strategy SHALL be stored in `decisions` with its
question, source, value, options, probabilities, confidence, model, model
version, latency, reasons, whether it acted, and the envelope version
(`1 + CONTEXT_VERSION` of the triage context). The rules SHALL be the
shared email signals and the questions the shared triage questions of
`campbooks_ai::decide`; this module SHALL hold no rules of its own. The model pass
(`conversations.decide_message`, role `decide`) SHALL ask the six questions
(`stream`, `needs_reply`, `asks_action`, `has_deadline`, `urgency`,
`suspicious`) in one call per inbound message, telling the model whose inbox
the message is in, only when the `decide` role is configured and has passed
its answers check, only for messages sent within
`CAMPBOOKS_CONVERSATIONS__DECIDE_BACKFILL_DAYS` (default 14), and once per
message. When the role has not passed its answers check, the pass SHALL do
exactly what it does with no `decide` role: store nothing, fail no job.
Answers the policy accepts SHALL be marked `acted`; no model answer
SHALL be accepted by the shipped policy. Provider
failures SHALL be retried, recording only the kind of failure.

#### Scenario: A confident but wrong model answer
- **WHEN** the model answers `primary` at 0.97 for a newsletter the rules placed in `newsletters`
- **THEN** the answer is stored with `acted = false` and the conversation stays in `newsletters`

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

### Requirement: Suggestions with their reasons
A conversation summary SHALL carry `suggestion` when the newest stream answer
on record for it (the model's before the rules' for the same message; a
model's answer only when its `model_version` starts with `checked:`) did
not act, names another stream than the conversation's, and no member placed
the conversation; the suggestion SHALL carry the stream, its source, its
confidence (0 to 100, when known) and its stored reasons. A conversation's
detail SHALL carry `decisions`: for each question, the answer about its newest
inbound message, with its source, confidence, reasons and whether it acted,
read the same way. Model answers without that stamp SHALL be kept and never
read. Neither SHALL change where a conversation is.

#### Scenario: The shipped policy
- **WHEN** the rules answer `newsletters` for a message under `Policy::baseline`
- **THEN** the conversation stays in `primary`, its summary suggests `newsletters` with the rules' reasons, and its detail lists the stream answer with `acted = false`

#### Scenario: A member moved it
- **WHEN** a member moves that conversation to `newsletters`
- **THEN** the summary the move returns has no suggestion

#### Scenario: Model answers from before the answers check
- **WHEN** a message's model answers carry `model_version` `laya-0.3.21-unchecked`
- **THEN** its detail lists the rules' answers, and the model's answers stay stored

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
SHALL raise a `review` need with trust `check`. Only model answers whose
`model_version` starts with `checked:` SHALL count. The need SHALL carry the due
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

#### Scenario: Unchecked model answers
- **WHEN** a message's only `needs_reply` true is a model answer marked `laya-0.3.21-unchecked`
- **THEN** reading it raises no `check` need, and an open `check` need it raised before is withdrawn as `superseded` on the next refresh
