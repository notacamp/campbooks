## ADDED Requirements

### Requirement: Kept inboxes are read as history
An inbox `conversations.list_inboxes` lists as `read_only` (its mailbox was
disconnected and its mail kept), or one the person only reads, SHALL open in
the reader with no action that writes: no Done, star, read or unread, move,
reply, reply all or forward, no assistant choices, and opening it SHALL NOT
mark it read. Until the inbox is known (its list loading or failed), the
reader SHALL offer no write action either. A kept inbox SHALL say so ("Kept as history", naming its
address). New messages SHALL never offer a read-only inbox to send from.

#### Scenario: A kept conversation
- **WHEN** the person opens a conversation of an inbox listed with `read_only: true`
- **THEN** "Kept as history" is shown, no write action is offered, E does nothing, and nothing is marked read
