## ADDED Requirements

### Requirement: The note says when Scout could not read the message
When `conversations.get` says `need_read_failed`, the reader's note for an open
need SHALL show one muted line, "Scout couldn't read this one.", under the
need's sentence, in the user's language (English, European Portuguese,
Spanish from Spain and French). It SHALL show nothing for a read that is
pending, finished, or has a call left.

#### Scenario: A read that failed for good
- **WHEN** the user opens a conversation with an open need whose read failed on every call
- **THEN** the note shows the need's sentence and, under it, "Scout couldn't read this one."

#### Scenario: A read that did not fail
- **WHEN** the user opens a conversation with an open need whose read is done or pending
- **THEN** the note shows no such line
