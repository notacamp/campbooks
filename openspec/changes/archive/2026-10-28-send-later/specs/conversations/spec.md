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
