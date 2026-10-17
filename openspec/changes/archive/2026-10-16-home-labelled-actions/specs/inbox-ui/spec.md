## MODIFIED Requirements

### Requirement: Streams, not people
The inbox SHALL be its streams: Primary, Updates, Receipts and Newsletters,
listed under Inbox in the sidebar (chips on phones) with their unread counts,
opened with G then 1 to 4. A stream's page SHALL list its pinned
conversations first, then the rest newest first, a page at a time, with
sender, time, subject, snippet, unread, pinned and message count, and All or
Unread.

#### Scenario: Open a stream from the keyboard
- **WHEN** the user presses G then 3
- **THEN** the Receipts stream opens

#### Scenario: A pinned conversation
- **WHEN** the user pins an older conversation in Primary
- **THEN** it moves to the top of Primary, marked pinned, and stays there as newer mail arrives

### Requirement: Tidying with undo
Done (E, archive), Pin or unpin (P, the provider's star), read or unread (U)
and move to a stream (M, then 1 to 4) SHALL act on the open conversation, or
on the list's cursor (J/K), show at once, and push an entry that Undo (Z, or
the toast) takes back. After Done the next conversation opens (the list, on
phones). The UI SHALL call it Pin everywhere, in en, pt-PT, es-ES and fr.

#### Scenario: Done and Undo
- **WHEN** the user presses E on an open conversation, then Z
- **THEN** the conversation is archived, the toast says "Done · <subject>", and Z unarchives it

#### Scenario: Pin
- **WHEN** the user presses P on an open conversation
- **THEN** it is pinned, the toast says "Pinned · <subject>" with Undo, and Gmail shows it starred

### Requirement: Kept inboxes are read as history
An inbox `conversations.list_inboxes` lists as `read_only` (its mailbox was
disconnected and its mail kept), or one the person only reads, SHALL open in
the reader with no action that writes: no Done, Pin, read or unread, move,
reply, reply all or forward, no assistant choices, and opening it SHALL NOT
mark it read. Until the inbox is known (its list loading or failed), the
reader SHALL offer no write action either. A kept inbox SHALL say so ("Kept as history", naming its
address). New messages SHALL never offer a read-only inbox to send from.

#### Scenario: A kept conversation
- **WHEN** the person opens a conversation of an inbox listed with `read_only: true`
- **THEN** "Kept as history" is shown, no write action is offered, E does nothing, and nothing is marked read
