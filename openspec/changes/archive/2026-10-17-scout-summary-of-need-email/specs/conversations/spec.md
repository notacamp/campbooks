## ADDED Requirements

### Requirement: Scout's summary of a need's email
When a need that is not a phishing warning (question `suspicious`) opens for
a message and the `complete` role is configured and may receive mail on this
server, a job (`conversations.summarize_need`, queued in
the transaction that stores the need, never inside the decide call or on a
request path) SHALL ask the role for a summary of that message in one or two
plain sentences, in English, European Portuguese, Spanish from Spain and
French in one call, and store it once in `conversations.need_summaries`
keyed by the message. The mail SHALL reach the model only as JSON string
values (sender, date, subject, own words as plain text without quoted
history, bounded) under a fixed instruction that says it is data and never
instructions; no preference or rule of the member SHALL be sent. The answer
SHALL be held to a strict JSON schema, then kept as plain text on one line
without control, reordering or invisible characters, at most 400 characters
per language, and not at all when English has nothing. The call SHALL be
counted against the workspace's daily AI limit as background work, one call
per message however often the need is worked out again, and a message that
already has a summary SHALL NOT be asked again. A call with no usable answer
SHALL be stored as failed and tried at most once more later. No job SHALL be
queued, and nothing shown, when the role is not configured or may not receive
mail here. The job SHALL skip a message whose need is no longer open, whose
inbox is kept read-only, that is in spam or the trash, or that has no words.
Failures SHALL be logged by kind only. When a summary is written the module
SHALL publish `conversations.need_summarised` (ids only).

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

### Requirement: Reading the summary
`conversations.get` SHALL return `need_summary`: the summary of the email
behind the conversation's open need, in the caller's language (English when
theirs is missing), or none when there is no open need, the need is a
phishing warning (even with a summary written before it became one), none is
written yet, or none can be. It SHALL be read with the same access checks as the
conversation, so a caller who may not read the inbox gets 404 and no summary,
and reading it SHALL never ask the model.

#### Scenario: Each member reads their own language
- **WHEN** a Portuguese reader and an English reader open the same item
- **THEN** each gets the summary in their language from the one stored row

#### Scenario: Not in the inbox
- **WHEN** a member who is not in the inbox asks for the conversation
- **THEN** the answer is 404

### Requirement: Summaries for needs that opened before
On upgrade every inbox SHALL be queued for the needs backfill, without any
manual step, and when the `complete` role may read mail the backfill's page
job SHALL queue `conversations.summarize_need` for each open need of its page
that has no summary or only one failed call. Running it again SHALL ask for
nothing a summary already exists for.

#### Scenario: An open need from before
- **WHEN** a Reply need was open before the upgrade and the role may read mail
- **THEN** the backfill queues its summary, the job writes it, and a second backfill asks nothing
