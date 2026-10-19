## ADDED Requirements

### Requirement: The Sent view
The sidebar SHALL list "Sent" under the four streams, set slightly apart and
without an unread count, and phones SHALL show a "Sent" chip after the
streams. G then 5 SHALL open it from anywhere in the workspace. It SHALL be a
list at `/w/:workspaceId/inbox/sent` of the conversations returned by
`conversations.list` with `sent`, with the same reader and pinned reply box as
a stream. A row SHALL start with "To <name>" (the address when the recipient
has no name) from `sent_to`, show the time of the member's latest message
(`last_sent_at`), and carry a "Replied" mark when the conversation's newest
message is not the member's. Its snippet keeps the "You:" prefix. An empty
Sent SHALL say "Nothing sent yet". A message the member sends SHALL appear in
it once the provider returns it, through the change feed.

#### Scenario: Opening Sent
- **WHEN** a member presses G then 5
- **THEN** the Sent list opens, its rows reading "To Clara Nunes", ordered by the member's latest message

#### Scenario: They replied
- **WHEN** someone answered a conversation Sam started
- **THEN** its Sent row carries "Replied" and the conversation is back in Primary

#### Scenario: Nothing sent
- **WHEN** the member has written in no conversation
- **THEN** Sent says "Nothing sent yet"
