## MODIFIED Requirements

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
schema with 8,192 tokens allowed at low reasoning effort (the model reasons
within them; chosen from a measurement, because at the default effort and
4,096 tokens a reasoning model could spend the whole budget thinking and
write nothing), and a call that ran out of tokens before it wrote anything
SHALL be asked again once in the same job with twice the budget before it
counts, then the summary kept as plain text on one line without control, reordering
or invisible characters, at most 400 characters per language, and not at all
when English has nothing. The call SHALL be counted against the workspace's
daily AI limit as background work under `conversations.read_message`, one
call per message however often its need is worked out again or it is
opened, and a message that already has a read SHALL NOT be asked again. A
call with no usable answer SHALL be stored as failed and tried at most once
more later. A newer message that raises a need and is flagged SHALL be read
on its own, with its own two calls, whatever became of an older message's
read. The job SHALL skip a message whose inbox is kept read-only, that
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

#### Scenario: The model thinks for its whole budget
- **WHEN** the model stops at its token limit without writing anything
- **THEN** the same job asks again once with twice the budget and low effort, and that answer is the call's answer

#### Scenario: A newer message after a failed read
- **WHEN** a read failed for good and a newer flagged message raises the need
- **THEN** the newer message is read on its own, with its own two calls

#### Scenario: No role
- **WHEN** no `complete` role may receive mail on this server
- **THEN** no read job is queued and the conversation shows its email without a summary

#### Scenario: A summary that does not count down
- **WHEN** Scout summarises a sign-in email whose link is valid for 10 minutes
- **THEN** the instruction it was given forbids "expires in 10 minutes" and asks for "valid for 10 minutes after it was sent"

#### Scenario: The same bill again
- **WHEN** Águas do Vale's reminder is read while `OpenTasks` lists the open task "Pay the water bill" for that sender
- **THEN** the model is shown that task, and its ask may name it as the same one

### Requirement: Reading the summary
`conversations.get` SHALL return `need_summary`: the summary of the email
behind the conversation's open need, in the caller's language (English when
theirs is missing), or none when there is no open need, the need is a
phishing warning (even with a summary written before it became one), none is
written yet, or none can be. It SHALL also return `need_read_failed: true`
when the model's read of that email failed on every call it gets (so none
will be written), and nothing otherwise. It SHALL be read with the same access
checks as the conversation, so a caller who may not read the inbox gets 404
and no summary, and reading it SHALL never ask the model.

#### Scenario: Each member reads their own language
- **WHEN** a Portuguese reader and an English reader open the same item
- **THEN** each gets the summary in their language from the one stored row

#### Scenario: Not in the inbox
- **WHEN** a member who is not in the inbox asks for the conversation
- **THEN** the answer is 404

#### Scenario: A read that failed for good
- **WHEN** both calls for the read of the open need's message gave no usable answer
- **THEN** `need_read_failed` is true and `need_summary` is none

#### Scenario: A read with a call left
- **WHEN** the read failed once and may be asked again
- **THEN** `need_read_failed` is absent
