## MODIFIED Requirements

### Requirement: The decide pass records suggestions
Every answer of every strategy SHALL be stored in `decisions` with its
question, source, value, options, probabilities, confidence, model, model
version, latency, reasons, whether it acted, and the envelope version
(`1 + CONTEXT_VERSION` of the triage context). The rules SHALL be the
shared facts strategy and the questions the shared triage questions of
`campbooks_ai::decide`; this module SHALL hold no rules of its own. The model pass
(`conversations.decide_message`, role `decide`) SHALL ask the seven questions
(`stream`, `needs_reply`, `asks_action`, `has_deadline`, `urgency`,
`suspicious`, `money`) in one call per inbound message, telling the model whose inbox
the message is in and the facts as hints, only when the `decide` role is
configured and has passed its answers check, only for messages sent within
`CAMPBOOKS_CONVERSATIONS__DECIDE_BACKFILL_DAYS` (default 14), and once per
message. When the role has not passed its answers check, the pass SHALL do
exactly what it does with no `decide` role: store nothing, fail no job.
Answers the policy accepts SHALL be marked `acted`; the shipped policy
(`Policy::baseline`) SHALL let a model answer act from the operating point
where the committed eval report shows the model run holding its question's
target, and a question the report does not measure (`money`) SHALL only be
recorded. Provider failures SHALL be retried, recording only the kind of
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
`checked:` SHALL count. A need SHALL carry no due date, and `urgent` when an
accepted `urgency` answer is "today". Messages from the conversation's own
inbox SHALL never raise one.

#### Scenario: A question from a person
- **WHEN** Marta writes "Could you confirm Thursday's walkthrough?" and the policy accepts the model's `needs_reply` true
- **THEN** the conversation has a `sure` `reply` need with no due date

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

### Requirement: Resolving a need
`POST …/conversations/{conversation_id}/need/resolve`
(`conversations.resolve_need`, `conversations:write`) SHALL clear the open
need with outcome `done` or `not_needed`, naming the message the need the
member saw comes from (`message_id`): when the open need comes from another
message, nothing SHALL be recorded and the answer SHALL be 409
`need_changed` (the same for `confirm_need`); `not_needed` SHALL also store a
correction of the question that raised it (true to false; for a `pay` need,
`money` from `owed` to `not_owed`). For a `pay` need it SHALL also take
`already_paid`, which clears the need as `done` and stores nothing else, and
`still_to_pay`, which turns a `check` `pay` need into a `sure` one with basis
`member` and stores a correction of `money` from `owed` to `owed`; either
outcome on a need that is not `pay` SHALL answer 409 `not_a_payment`, and
`still_to_pay` on a `pay` need that is already `sure` SHALL answer 409
`no_open_need`. `…/need/confirm` (`conversations.confirm_need`) SHALL turn a
`check` need into a `sure` one and store a correction confirming the answer.
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

## ADDED Requirements

### Requirement: Reading mail again for the money question
On upgrade to the money question, the needs checks SHALL accept the
question `money` and the verdicts `already_paid` and `still_to_pay`, and
every inbox SHALL be queued for the needs backfill, without any manual step,
so each recent need is worked out again under the new rule (a `pay` need the
old rule raised is withdrawn as `superseded`). When a trusted `decide` role
is configured, the backfill SHALL ask the model about each conversation's
newest inbound message sent within the decide backfill window that lacks a
checked model answer to any triage question, once; the checked answers
already stored SHALL be kept and only the missing ones added. Running it
again SHALL ask nothing more for a message the model answered.

#### Scenario: A receipt that was Pay
- **WHEN** a payment confirmation in Receipts had an open `pay` need raised by `asks_action`, and the upgrade runs with Jev configured
- **THEN** the need is withdrawn as `superseded`, the model is asked about the message once, and since it answers `money` `paid` no need is raised again

#### Scenario: A bill that was Pay
- **WHEN** a bill had an open `pay` need raised by `asks_action`, and the model answers `money` `owed` at 0.93
- **THEN** after the upgrade the conversation has a `pay` need with trust `check` and question `money`

### Requirement: Labelling the owner's money mail
`campbooks-server labelling export --workspace <id> --days <n> --out <folder>`
SHALL write, for one workspace, the newest inbound message (not in spam or
the trash) of each conversation in Receipts or that had a `pay` need, sent
within the last `n` days (default 90), newest first and at most 500, as an
unlabelled eval set: `manifest.json` (a private set asking `money`),
`items/owner.jsonl` (what the decide model reads of each message: envelope,
list and automation headers, attachment names and media types, the start of
the text body; items alternating `dev` and `test`) and `labels.csv` (id,
date, sender, subject, attachments and an empty `money` column). It SHALL run
inside the workspace with row-level security, refuse a folder that exists,
and say that the folder holds real mail to keep off the repository. `cargo
xtask eval decide --set-dir <folder>` SHALL read such a set where it is,
fill each item's `money` label from the sheet, leave unlabelled items out of
the scores, skip the fictional set's reserved-domain and id checks for a
private set, and write its report into the folder.

#### Scenario: Exporting
- **WHEN** Sam's workspace holds a bill from Acme Hosting in Receipts, a question from Marta and a phishing message
- **THEN** the export lists only the bill, as `owner-001`, with empty labels

#### Scenario: Another workspace
- **WHEN** the export names another workspace
- **THEN** none of Sam's mail is written

#### Scenario: Labels from the sheet
- **WHEN** the owner writes `owed` in the `money` cell of one row and leaves another blank
- **THEN** the eval scores the first item against `owed` and leaves the second out
