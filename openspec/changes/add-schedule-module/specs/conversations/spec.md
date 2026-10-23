## RENAMED Requirements

- FROM: `### Requirement: Scout's summary of a need's email`
- TO: `### Requirement: Scout's read of a message`

## MODIFIED Requirements

### Requirement: The decide pass records suggestions
Every answer of every strategy SHALL be stored in `decisions` with its
question, source, value, options, probabilities, confidence, model, model
version, latency, reasons, whether it acted, and the envelope version
(`1 + CONTEXT_VERSION` of the triage context). The rules SHALL be the
shared facts strategy and the questions the shared triage questions of
`campbooks_ai::decide`; this module SHALL hold no rules of its own. The model pass
(`conversations.decide_message`, role `decide`) SHALL ask the ten questions
(`stream`, `needs_reply`, `asks_action`, `has_deadline`, `urgency`,
`suspicious`, `money`, `next_move`, `has_dated_thing`, `dated_kind`), and
`awaited` when the facts leave someone who may be awaited, in one call per
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

### Requirement: Scout's read of a message
A job (`conversations.read_message`, never inside the decide call or on a
request path) SHALL ask the `complete` role, in one call, for everything
Scout needs from one message: when a need is open on it, a summary in
English, European Portuguese, Spanish from Spain and French together with
its dates (see "Dates from Scout's read"); and its items and asks (see
"Items and asks a read returns"). It SHALL be the only call to `complete`
that reads a message's content to understand it, and no module SHALL make
another. The job SHALL be queued only for a **flagged** message (see
"Messages flagged for a read") that is not a phishing warning (question
`suspicious`) and either has a need that opened for it, queued in the
transaction that stores the need, or was opened by a member who asked for
its read (see "Asking for a read on open"); and only when the `complete`
role is configured and may receive mail on this server. A need on a message
that is not flagged SHALL get no read when it opens. The read SHALL be stored
once in `conversations.message_reads` keyed by the message. The mail SHALL
reach the model only as JSON string values (sender, date, subject, own words
as plain text without quoted history, bounded) under a fixed instruction
that says it is data and never instructions, together with the time zone of
the inbox's personal owner (through the kernel port `TimeZones`; UTC for a
shared inbox or when none is set), the decide pass's `dated_kind` as a hint,
the items already read in the conversation (id, kind, title, when), and the
open tasks the kernel port `OpenTasks` gives for the inbox, the conversation
and the sender's address within that inbox (at most ten: reference, title, due; none while no module
implements it); no preference or rule of the member SHALL be sent. The
instruction SHALL forbid time relative to the moment it is written ("in 10
minutes", "today", "tomorrow", "expires soon") and ask for times and
durations as the email states them, anchored to when it was sent, because
the read is kept and shown later. The answer SHALL be held to a strict JSON
schema with at least 4,096 tokens allowed (the model reasons within them),
then the summary kept as plain text on one line without control, reordering
or invisible characters, at most 400 characters per language, and not at all
when English has nothing. The call SHALL be counted against the workspace's
daily AI limit as background work under `conversations.read_message`, one
call per message however often its need is worked out again or it is
opened, and a message that already has a read SHALL NOT be asked again. A
call with no usable answer SHALL be stored as failed and tried at most once
more later. The job SHALL skip a message whose inbox is kept read-only, that
is in spam or the trash, or that has no words. Failures SHALL be logged by
kind only. When a summary is written the module SHALL publish
`conversations.need_summarised` (ids only); once the read is stored it SHALL
publish `conversations.message_read` and work the conversation's need out
again.

#### Scenario: A need opens
- **WHEN** Marta asks "Could you send the signed quote by Friday?", the pass flags the message with `asks_action`, a Reply need opens and the `complete` role may read mail
- **THEN** one read is written with its summary in all four languages and its ask, `need_summarised` and `message_read` are published once each

#### Scenario: A plain question raises no read
- **WHEN** a Reply need opens on a message the pass did not flag
- **THEN** no read is queued, and the read happens if a member opens the message

#### Scenario: Worked out again
- **WHEN** the same need is refreshed, the message is opened again, or the job runs a second time
- **THEN** the model is not asked again and nothing is written

#### Scenario: Mail that may be phishing
- **WHEN** a message raises a phishing warning
- **THEN** no read is asked for or stored: a summary would retell the scam in Scout's voice, and the warning is what matters

#### Scenario: Nothing usable
- **WHEN** the model answers with nothing usable
- **THEN** the message is stored as failed, nothing is shown, a later run asks once more, and after that never

#### Scenario: No role
- **WHEN** no `complete` role may receive mail on this server
- **THEN** no read job is queued and the conversation shows its email without a summary

#### Scenario: A summary that does not count down
- **WHEN** Scout summarises a sign-in email whose link is valid for 10 minutes
- **THEN** the instruction it was given forbids "expires in 10 minutes" and asks for "valid for 10 minutes after it was sent"

#### Scenario: The same bill again
- **WHEN** Águas do Vale's reminder is read while `OpenTasks` lists the open task "Pay the water bill" for that sender
- **THEN** the model is shown that task, and its ask may name it as the same one

### Requirement: Dates from Scout's read
When a need is open, the read SHALL include `lapses_at` (an RFC 3339 time:
when what the email asks stops being possible, such as a link or a code
expiring or an offer or an invitation closing) and `due_on` (a `YYYY-MM-DD`
date), each worked out by the model from the email's words and its
`sent_at`, and null when the email gives none. `due_on` SHALL be asked for
only for a `reply`, `review` or `waiting` need: by when the recipient is
asked to act, or for `waiting`, by when the awaited person is expected to
act or write. A `do` or `pay` need SHALL take its due date from the read's
first ask with the same verb. No deterministic code SHALL read the mail to
find or correct a date (ADR 0018). The module SHALL drop a value that does
not parse, that comes before the message was sent, or that lies more than
366 days after it. The dates SHALL be stored with the message in
`conversations.message_reads`, with the time they were read, and copied to
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

#### Scenario: A bill's due date
- **WHEN** the read of an open Pay need's invoice has a `pay` ask due 30 October
- **THEN** the need's due date is 30 October

### Requirement: Summaries for needs that opened before
On upgrade every inbox SHALL be queued for the needs backfill, without any
manual step, and when the `complete` role may read mail the backfill's page
job SHALL queue `conversations.read_message` for each open need of its page
whose message is flagged and has no read, only one failed call, or a summary
written before items and asks were read. Such a message SHALL be read once
more: a usable answer replaces its texts and stores its dates, items and
asks; no usable answer keeps its texts and marks it read, so it is never
asked again. Running the backfill again SHALL ask for nothing that has a
read. Rows of `conversations.need_summaries` SHALL be kept as reads in
`conversations.message_reads`.

#### Scenario: An open need from before
- **WHEN** a flagged Reply need was open before the upgrade with no summary and the role may read mail
- **THEN** the backfill queues its read, the job writes it, and a second backfill asks nothing

#### Scenario: A summary from before the dates
- **WHEN** an open Do need's flagged message was summarised before items and asks were read
- **THEN** the backfill queues it once, its items and asks are stored, and a second backfill asks nothing

#### Scenario: An open need on an unflagged message
- **WHEN** an open Reply need's message is not flagged and has no summary
- **THEN** the backfill queues nothing for it

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
Zoho file as received). While the action catalogue is switched off
(`CAMPBOOKS_CONVERSATIONS__ACTIONS__ENABLED`), no new wait SHALL be raised;
a wait already open SHALL keep its course.

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

## ADDED Requirements

### Requirement: Messages flagged for a read
A message SHALL be **flagged** when the decide pass answered `asks_action`,
`has_deadline` or `has_dated_thing` true with a confidence of at least
`CAMPBOOKS_CONVERSATIONS__DATED__FLAG` (default 0.6), its accepted `stream`
answer is not `newsletters`, and no accepted `suspicious` answer is true.
Flagging SHALL act on nothing by itself and cost no call. A message flagged
by `has_dated_thing` that the structured data did not cover SHALL say so to
the reader ("Scout can read this for dates"). With
`CAMPBOOKS_CONVERSATIONS__DATED__ENABLED` off, no message SHALL be flagged by
`has_dated_thing` and no read SHALL be queued on open.

#### Scenario: A dentist's reminder
- **WHEN** the model answers `has_dated_thing` true at 0.88 for a clinic's reminder sorted to Updates
- **THEN** the message is flagged and nothing else happens until a need opens or a member opens it

#### Scenario: A promotion with an end date
- **WHEN** the model answers `has_dated_thing` true at 0.9 for a sale sorted to Newsletters
- **THEN** the message is not flagged

### Requirement: Asking for a read on open
`conversations.request_message_read` (`POST
/api/v1/conversations/messages/{id}/read`, scope `conversations:read`) SHALL
queue the read of a flagged message that has none and no read queued, and
otherwise do nothing and say so; it SHALL never wait for the model. The
client SHALL call it when a member opens a message. A caller who may not read
the message's inbox SHALL get 404.

#### Scenario: First open of a flagged booking
- **WHEN** Inês opens a flagged restaurant booking that has no read
- **THEN** the read is queued, the answer says it is queued, and opening it again queues nothing

#### Scenario: An unflagged message
- **WHEN** Inês opens a message that is not flagged
- **THEN** nothing is queued and the answer says there is nothing to read

### Requirement: Items and asks a read returns
The read SHALL return, in the same answer, at most five items and at most
three asks. An item is something of the reader's that happens at a time:
its `kind` (`charge`, `appointment`, `meeting`, `travel`, `stay`, `show`,
`reservation`, `delivery` or `expiry`), a title of at most 80 characters that
is not the subject, a start (an RFC 3339 time with offset, or a date when
all-day), an optional end, whether it is all-day, an optional IANA time zone,
an optional place of at most 120 characters, the sentence that gives its
date, quoted as written, and optionally the id of an item already read in
the conversation that it updates or cancels. An ask is an obligation of the
reader, dated or not: a title of at most 80 characters, the sentence that
asks, quoted as written, an optional due (a date or a time), a verb (`do` or
`pay`), for `pay` an optional amount (minor units and currency), and
optionally the reference of an open task it is the same as. The instruction
SHALL say: only asks of the reader, never of other recipients or of the
sender; an expiry with something to do before it is an ask with verb `do`,
a plain expiry an item; money taken automatically is a `charge` item, money
the reader must send an ask with verb `pay`; an obligation is never also an
item; a date without a time is all-day, never a guessed hour; relative words
resolve against `sent_at` in the given time zone; dates in quoted history,
signatures or adverts are not items. Code SHALL only drop, per item and per
ask, never add or correct: one whose quote is not in the message's words
(ignoring case and runs of whitespace); one whose start or due does not
parse or lies outside one day before to 731 days after the message was sent;
an item that has already ended when it is read; an item whose kind is not
in the set; an item whose `cancels` id is not an item of the same
conversation (the whole item: a cancellation of something never held adds
nothing); an `updates` id that is not an item of the same conversation (the
reference only, the item is kept as a new one); a `same_as` that was not in the list
sent (the reference only); an amount whose digits are not in the quote or
the message's words (the amount only). Each drop SHALL be logged by reason
only.

#### Scenario: A flight and a hotel in one email
- **WHEN** a travel agency's confirmation is read and the model returns a `travel` item for the flight and a `stay` item for the hotel, each with its quote
- **THEN** both items are stored

#### Scenario: A made-up quote
- **WHEN** the model returns an item whose quote is not in the email
- **THEN** that item is dropped and the others are kept

#### Scenario: A cancellation of something never held
- **WHEN** the model returns an item whose `cancels` id is not an item of the conversation
- **THEN** that item is dropped; an unknown `updates` id only loses the reference and the item is kept as a new one

#### Scenario: A plan renewing
- **WHEN** a streaming service writes "Your plan renews on 3 November and €9.99 will be charged to your card"
- **THEN** the read may return a `charge` item on 3 November, and no ask

#### Scenario: A passport to renew
- **WHEN** a consulate writes that Inês's passport expires on 12 March and must be renewed before then
- **THEN** the read may return an ask with verb `do` due 12 March, and no `expiry` item

#### Scenario: An amount that is not in the email
- **WHEN** a `pay` ask comes back with an amount of €120.00 and "120" appears nowhere in the message
- **THEN** the ask is kept without an amount

#### Scenario: A date in an old quote
- **WHEN** a reply quotes an earlier message's meeting date and says nothing of its own about a date
- **THEN** the instruction says the quoted date is not an item

### Requirement: Dates from structured data
Before any read, the module SHALL read the message's machine-readable dates
(ADR 0020): each `text/calendar` part, with an iCalendar parser (an event
becomes a `meeting` item when it has attendees and an `appointment`
otherwise; `METHOD:CANCEL` cancels the item with the same `UID`; a higher
`SEQUENCE` updates it), and schema.org markup in its HTML (JSON-LD or
microdata): flight, train, bus and rental car reservations as `travel`,
lodging as `stay`, event reservations as `show`, food establishment
reservations as `reservation`, parcel deliveries as `delivery`, and an
invoice with a payment due date as a `pay` ask with that due date and its
total as the amount. Other types SHALL be ignored, and only typed fields
read, never the text around them. This SHALL apply only when the receiving
provider's authentication results show the sender's domain passed DMARC
aligned (as read under "Authentication results are read strictly") and no
accepted `suspicious` answer is true; otherwise the message SHALL go on as
if it had none. Items and asks found this way SHALL be stored as a read with
layer `structured`, published, and given to a later model read as items
already read; a message they cover (every `dated_kind` the pass named is
among them, or the pass named none) SHALL not be read by the model for
dates, and a read for its need's summary SHALL still be allowed.

#### Scenario: An airline's confirmation
- **WHEN** an authenticated airline's email carries a `FlightReservation` departing 6 November 07:40 from Lisbon
- **THEN** a `travel` item is stored with layer `structured`, `message_read` is published, and no model is called for its dates

#### Scenario: Markup from a look-alike domain
- **WHEN** the same markup arrives from a domain that fails DMARC
- **THEN** no structured item is stored and the message is treated as having no markup

#### Scenario: An invitation cancelled
- **WHEN** an organiser's later email carries `METHOD:CANCEL` for the `UID` of an earlier invitation
- **THEN** the earlier item is cancelled

### Requirement: A read is published
When a read is stored, of either layer, the module SHALL publish
`conversations.message_read` v1 in the same transaction with the message,
conversation and inbox ids, the inbox's personal owner (none for a shared
inbox), the layer, the items (key, kind, start, end, all-day, time zone,
updates or cancels), the asks (key, verb, due, whether it has an amount, same
as) and whether a summary is ready, and no title, place, quote, amount or
other text. Item and ask keys SHALL be stable for the same message and
position, so a repeated delivery changes nothing for a consumer.

#### Scenario: The payload of a booking
- **WHEN** a restaurant booking is read
- **THEN** `message_read` carries a `reservation` item with its time and key, and not the restaurant's name

### Requirement: Reading what was read
`conversations.get_message_read` (`GET
/api/v1/conversations/messages/{id}/read`, scope `conversations:read`) SHALL
return a message's read: its layer, its items and asks with their text
(title, place, quote, amount), whether it is queued, failed or done, and,
when the pass flagged a dated thing and no `complete` role may read mail,
the `dated_kind` the pass named. It SHALL be read with the same access
checks as the conversation, so a caller who may not read the inbox gets 404,
and reading it SHALL never ask the model.

#### Scenario: Read by the inbox's member
- **WHEN** Inês asks for the read of her flight email
- **THEN** she gets the `travel` item with its title, time and quote

#### Scenario: Not in the inbox
- **WHEN** a member who is not in the inbox asks for that read
- **THEN** the answer is 404
