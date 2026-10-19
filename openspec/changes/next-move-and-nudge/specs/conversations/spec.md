## MODIFIED Requirements

### Requirement: The decide pass records suggestions
Every answer of every strategy SHALL be stored in `decisions` with its
question, source, value, options, probabilities, confidence, model, model
version, latency, reasons, whether it acted, and the envelope version
(`1 + CONTEXT_VERSION` of the triage context). The rules SHALL be the
shared facts strategy and the questions the shared triage questions of
`campbooks_ai::decide`; this module SHALL hold no rules of its own. The model pass
(`conversations.decide_message`, role `decide`) SHALL ask the eight questions
(`stream`, `needs_reply`, `asks_action`, `has_deadline`, `urgency`,
`suspicious`, `money`, `next_move`), and `awaited` when the facts leave
someone who may be awaited, in one call per inbound message, telling the
model whose inbox the message is in and the facts as hints, only when the
`decide` role is configured and has passed its answers check, only for
messages sent within `CAMPBOOKS_CONVERSATIONS__DECIDE_BACKFILL_DAYS`
(default 14), and once per message. When the role has not passed its answers
check, the pass SHALL do exactly what it does with no `decide` role: store
nothing, fail no job. Answers the policy accepts SHALL be marked `acted`; the
shipped policy (`Policy::baseline`) SHALL let a model answer act from the
operating point where the committed eval report shows the model run holding
its question's target, and a question the report does not measure (`money`,
`next_move`, `awaited`) SHALL only be recorded. An answer to `next_move` or
`awaited` that is missing or outside its options SHALL record nothing and
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

## ADDED Requirements

### Requirement: Who has the next move
`next_move` SHALL be a choice of `recipient`, `sender`, `someone_else` and
`nobody`: who is expected to act next on the conversation after this
message. `awaited` SHALL be a choice over the people the facts leave (at most
six, `p1` to `p6` in the facts' order, each shown to the model with name,
address and whether in To or Cc) and `none`, asked only when there is at
least one. When the committed eval report measures `next_move` and the policy
accepts an answer other than `recipient`, a model's `needs_reply` or
`asks_action` answer SHALL raise no `reply` or `do` need for that message;
the facts' and taught rules' answers SHALL still raise theirs. Until then
`next_move` SHALL never take away a need.

#### Scenario: Asked to write to me
- **WHEN** João writes to Ana Costa, copying the inbox: "Ana, could you send Sam the floor plan?"
- **THEN** the model is asked `next_move` and `awaited` with one option, Ana Costa, and `none`

#### Scenario: Measured later
- **WHEN** the policy accepts `next_move` `someone_else` for a message whose model `needs_reply` true is also accepted
- **THEN** no `reply` need is raised from it

#### Scenario: Not measured yet
- **WHEN** the policy does not accept `next_move` and accepts the model's `needs_reply` true
- **THEN** the `reply` need is raised as before

### Requirement: Waiting on someone
When the newest inbound message raises no other need, its accepted `stream`
answer is `primary` or no `stream` answer is accepted, and no fact marks it as
list, automatic or no-reply mail, the conversation SHALL get a `waiting`
need on one person: the sender, when the policy accepts `next_move` `sender`
or the model answers it with a confidence of at least 0.8; or the person a
model `awaited` answer names (not `none`), when the policy accepts
`next_move` `someone_else` or the model answers it with a confidence of at
least 0.8. The need SHALL hold that person's name and address as the headers
gave them, trust `sure` when the policy accepted `next_move` and `check`
otherwise, and a nudge time: when Scout's read of the email gives a `due_on`
date, the working day after it, else the message's sent time plus
`CAMPBOOKS_CONVERSATIONS__WAIT__WORKING_DAYS` (default 3; 1 to 15, else the
server refuses to start) working days, working days being Monday to Friday by
UTC date, at the time of day the message was sent. Its due date SHALL be the
nudge time's date. The need clock SHALL wake it at the nudge time and turn it
into a `nudge` need, with a new revision. Only model answers whose
`model_version` starts with `checked:` SHALL count.

#### Scenario: Ana was asked to write
- **WHEN** João's message of Wednesday 14 October 10:00 asks Ana Costa to send the floor plan, raises no other need, and the model answers `next_move` `someone_else` at 0.91 and `awaited` Ana Costa
- **THEN** the conversation has a `check` `waiting` need on Ana Costa with the nudge time Monday 19 October 10:00

#### Scenario: A promise with a date
- **WHEN** Rui writes "I'll send the signed contract by Friday 16 October", the model answers `next_move` `sender` at 0.86, and Scout's read gives `due_on` 16 October
- **THEN** the need waits on Rui with the nudge time Monday 19 October, at the time Rui's message was sent

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

### Requirement: When a wait ends
An open `waiting` or `nudge` need SHALL be cleared as `heard_back` when an
inbound message from its awaited address (without regard to case), sent after
the need's message, reaches the same inbox, in this conversation or in any
other. A message sent from the inbox with the awaited address in `To` or
`Cc`, after the need's message, SHALL set it back to `waiting` with a new
nudge time worked out from that message's sent time, counting the nudges.
Archiving a conversation SHALL NOT clear a wait; trash, spam and a member
resolving it SHALL, as for any need, and a newer inbound message from anyone
else in the conversation SHALL derive the need again from it.

#### Scenario: Ana writes in a new thread
- **WHEN** Ana Costa writes to the inbox in a new conversation, "Floor plan attached"
- **THEN** the wait on her in João's conversation is cleared as `heard_back`

#### Scenario: The member nudged her
- **WHEN** Sam sends Ana a nudge on Monday 19 October 11:00
- **THEN** the need is `waiting` again, nudges 1, with the nudge time Thursday 22 October 11:00

#### Scenario: Archived while waiting
- **WHEN** Sam archives João's conversation
- **THEN** the wait goes on and Home still nudges on Monday

### Requirement: Who a need waits on
A conversation summary's `need` SHALL carry `waiting_on` (the awaited name
and address) when the need is `waiting` or `nudge`, and the number of nudges
sent. A `waiting` need SHALL never be counted in a stream's or an inbox's
"needs you" figures.

#### Scenario: Home asks for its rows
- **WHEN** the client lists João's conversation, whose need is `nudge`
- **THEN** its summary's need carries `waiting_on` Ana Costa and her address, and nudges 0

### Requirement: Nudge drafts
Creating a draft of kind `nudge` for a message SHALL be allowed only when the
conversation's open need is `waiting` or `nudge` (else 409 `not_waiting`) and
SHALL fill on the server, as a reply all of the need's message: the awaited
person in `To`, everyone else reply all would address in `Cc`, the inbox's
own address in neither, no duplicates, with the subject, `In-Reply-To`,
`References` and thread key of a reply. Every other rule of drafts SHALL
apply.

#### Scenario: Nudging Ana
- **WHEN** Sam starts a nudge on João's message to Ana Costa that copied Sam
- **THEN** the draft is to Ana Costa, copies João, holds Sam's address nowhere, and its subject starts with `Re: `

#### Scenario: Nothing to wait for
- **WHEN** Sam starts a nudge on a conversation whose need is `reply`
- **THEN** the response is 409 `not_waiting` and no draft exists

### Requirement: Scout writes the nudge
`POST …/conversations/{conversation_id}/nudge/draft`
(`conversations.write_nudge`) SHALL answer a short, friendly follow-up to the
awaited person about what they were expected to do, in the thread's
language, signed with the member's name, or the member's draft rewritten
shorter or as a plain-words change asks, with the rules of
`conversations.write_reply`: `conversations:send`, 404 to anyone who may not
read the conversation, 403 `inbox_read_only`, only the `complete` role and
only where it may receive mail (else 409 `reply_unavailable` before reading
the thread), the daily AI limit as interactive and the per-member hourly
allowance (429 `rate_limited`), 503 `reply_failed` logged by kind only, mail
as data, and nothing changed: no draft, no stored text, no event. It SHALL
answer 409 `not_waiting` when the conversation's need is neither `waiting`
nor `nudge`. The instruction SHALL forbid inventing facts, dates or
pressure.

#### Scenario: A nudge for Ana
- **WHEN** Sam asks for the nudge on João's conversation
- **THEN** the response is a few lines to Ana asking whether she had a chance to send the floor plan, signed Sam, and no draft exists

#### Scenario: No model may read mail
- **WHEN** the `complete` role is not configured
- **THEN** the response is 409 `reply_unavailable` and no request reaches a model

### Requirement: Waits for mail already here
On upgrade, every inbox SHALL be queued once to ask `next_move` and
`awaited` about its inbound messages within the needs window whose
conversation holds no open need and whose other answers are on record, within
the workspace's daily AI limit; the needs SHALL then be worked out again.
Messages SHALL not be asked their other questions again.

#### Scenario: João's email from last week
- **WHEN** the upgrade runs and João's message of 14 October, which raised no need, has no `next_move` answer
- **THEN** it is asked `next_move` and `awaited` once, and the conversation waits on Ana from the answers
