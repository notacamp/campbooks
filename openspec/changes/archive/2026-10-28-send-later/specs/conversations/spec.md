## ADDED Requirements

### Requirement: Sending at a chosen time
`conversations.send_draft` SHALL take an optional `send_at`. When given, it
SHALL be at least a minute after the undo window ends and at most 366 days
ahead. Otherwise the request SHALL answer 400 `send_at_invalid` and leave the
draft unchanged. A valid `send_at` SHALL schedule the draft to leave then,
and it SHALL be cancellable until then with `conversations.cancel_send`,
which makes it an editable draft again. Without `send_at`, a send SHALL wait
only for the undo window.

#### Scenario: Tomorrow morning
- **WHEN** Sam sends a draft with `send_at` tomorrow at 08:00
- **THEN** the draft is scheduled with that `send_at`, and nothing is sent before then

#### Scenario: In the past
- **WHEN** Sam sends a draft with a `send_at` an hour ago
- **THEN** the response is 400 `send_at_invalid` and the draft is still a draft

#### Scenario: Taken back the evening before
- **WHEN** Sam cancels a send scheduled for tomorrow
- **THEN** it is a draft again, and nothing is ever sent for that schedule

### Requirement: A scheduled send that stops says why
When a scheduled send is due and cannot be released, nothing SHALL be sent and the draft SHALL become `failed` with a reason: `destination_changed` when its inbox no longer sends from where the request was bound, `sender_may_not_send` when the author lost the workspace or the right to send from the inbox. Only the generation the release was queued for SHALL change. Drafts SHALL keep it listed.

#### Scenario: Removed before it was due
- **WHEN** Sam's send is scheduled for tomorrow and Sam is made a reader of the inbox before then
- **THEN** nothing is sent, and the draft is failed with `sender_may_not_send` and still listed

#### Scenario: The mailbox changed
- **WHEN** the inbox's mail goes out from another mailbox by the time a send is due
- **THEN** nothing is sent and the draft is failed with `destination_changed`

### Requirement: Overdue scheduled sends are settled
A sweep every ten minutes SHALL mark `failed` with `release_lost` each draft still scheduled fifteen minutes past its time with no unfinished release job for its generation. It SHALL NOT queue the release again, because that job may already have sent. A draft whose release job is queued, or that is not yet late, SHALL be left alone.

#### Scenario: The job was lost
- **WHEN** a scheduled draft is an hour overdue and its release job is gone
- **THEN** it becomes failed with `release_lost`, and no send is queued

#### Scenario: The workers are behind
- **WHEN** a scheduled draft is an hour overdue and its release job is still queued
- **THEN** the sweep leaves it for the job

### Requirement: Limit on sends for one minute
A send with a chosen `send_at` SHALL be refused with 429 `send_at_busy` when the author already has as many sends scheduled for the same minute as the Send rate allows in a minute (its count over its window, at least one), counted for the author in the workspace. Other minutes and other authors SHALL be unaffected.

#### Scenario: A fourth at the same minute
- **WHEN** Sam has three sends scheduled for Tuesday 08:00 and schedules a fourth for it
- **THEN** the response is 429 `send_at_busy` and the fourth is still a draft
