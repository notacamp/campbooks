## ADDED Requirements

### Requirement: Who writes to the caller
`GET /api/v1/w/{workspace_id}/senders` (`conversations.list_senders`,
`conversations:read`, callable by the assistant) SHALL list senders of
inbound mail in the inboxes the caller can read: address, display name,
messages received and messages the caller's inboxes replied to; never a
subject or any text. Without `q` it SHALL return the 10 senders the
caller's inboxes replied to most (then received most); with `q` (2 to 100
characters) it SHALL return up to 10 senders whose address, display name
or domain contains each of its words, case- and accent-insensitively, in
the same order. `limit` SHALL be 1 to 10 (else 400). There SHALL be no
cursor. A sender seen only in inboxes the caller cannot read SHALL never
appear. It SHALL read only the newest inbound mail: the last 365 days, at
most 5,000 messages (through a partial index on received mail), so the
counts are within that window and its cost doesn't grow with an inbox's
history. Within one compile, the assistant SHALL ask it at most once per
`q`.

#### Scenario: Suggestions for "always reach you"
- **WHEN** a member lists senders without `q`
- **THEN** the senders they replied to most come first, with their counts and no subjects

#### Scenario: Another member's inbox
- **WHEN** a sender wrote only to an inbox the caller is not in
- **THEN** that sender is not listed, for that caller or for a client token without the inbox

#### Scenario: Old mail only
- **WHEN** a sender's every message is more than a year old
- **THEN** that sender is not listed

### Requirement: The caller's labels
`GET /api/v1/w/{workspace_id}/labels` (`conversations.list_labels`,
`conversations:read`, callable by the assistant) SHALL list the labels of
the inboxes the caller can read: id, inbox and name.

#### Scenario: Only readable inboxes
- **WHEN** a member lists labels and another inbox has a label "Board"
- **THEN** "Board" is not listed

### Requirement: Shared inboxes are marked
`conversations.list_inboxes` SHALL say of each inbox whether it is
`shared`: whether anyone other than the caller holds a role on it. It
SHALL never say who.

#### Scenario: Sam's own inbox and the office inbox
- **WHEN** Sam lists his inboxes and Ana also reads office@
- **THEN** his own inbox says `shared` = false and office@ says `shared` = true
