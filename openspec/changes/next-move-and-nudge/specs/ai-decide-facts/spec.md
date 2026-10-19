## ADDED Requirements

### Requirement: Who may be awaited
The facts SHALL work out who on an email may be awaited, only by removing
people: from the email's `To` then `Cc`, in order, leave out the inbox's own
addresses, the sender, and every list, no-reply or service address
(`is_machine_sender`), drop duplicates without regard to case, and keep at
most six. A sender that is a list, no-reply or service address SHALL never
be awaited either. The facts SHALL answer neither `next_move` nor `awaited`:
whose turn it is is for the model to read.

#### Scenario: A colleague and a no-reply address
- **WHEN** João writes to Ana Costa, copying the inbox and `no-reply@tickets.example.com`
- **THEN** Ana Costa is the only person who may be awaited, and neither question is answered by the facts

#### Scenario: Nobody to pick
- **WHEN** Marta writes only to the inbox
- **THEN** nobody may be awaited and the `awaited` question is not asked
