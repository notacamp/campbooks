## ADDED Requirements

### Requirement: Who writes to the caller
`GET /api/v1/w/{workspace_id}/conversations/senders`
(`conversations.list_senders`, `conversations:read`) SHALL list the senders
of inbound mail in the inboxes the caller can see: address, display name,
registrable domain, messages received, messages the caller's inboxes
replied to, messages opened, and the newest message's time. It SHALL accept
`q` (matched case- and accent-insensitively against words of the address,
display name and domain), `sort` (`replied`, `received` or `unopened`), an
opaque cursor and a limit of 1 to 50 (else 400). A sender seen only in
inboxes the caller cannot see SHALL never appear.

#### Scenario: Suggestions for "always reach you"
- **WHEN** a member lists senders sorted by `replied`
- **THEN** the senders they replied to most come first, with their counts

#### Scenario: Another member's inbox
- **WHEN** a sender wrote only to an inbox the caller is not in
- **THEN** that sender is not listed

### Requirement: The decide pass reads the inbox owner's preferences
`conversations.decide_message` SHALL read the preferences block of the
inbox owner through the kernel's `MemberPreferences` port and pass it in
the decision context's preferences slot, and SHALL store the block's schema
and revision with each decision. No block SHALL mean an empty slot.

#### Scenario: A revised profile
- **WHEN** the owner changes an answer and new mail arrives
- **THEN** its decisions record the new revision, and earlier decisions keep theirs
