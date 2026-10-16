## MODIFIED Requirements

### Requirement: Spam and trash need nothing
A message in spam or the trash SHALL never raise a need: a conversation's
need SHALL come from its newest inbound message that is in neither, and a
need whose own message moves to spam or the trash SHALL be cleared as
`trashed`. Working out a need SHALL hold the messages it can come from
against a concurrent move to spam or the trash (taking them before the need's
own row), so that no open need is stored for a message after its move
committed. A message that leaves spam or the trash SHALL raise its need again
as any message does; a member's Done or Not needed SHALL still hold, and a
member's confirmation SHALL still make the reopened need theirs (basis
`member`).

#### Scenario: The lease goes to the trash
- **WHEN** the message Pedro's `do` need comes from is moved to the trash, while his later message stays
- **THEN** the need is cleared with cause `trashed`

#### Scenario: A question in spam
- **WHEN** a question lands in spam inside a conversation whose need comes from an earlier message
- **THEN** the need still comes from the earlier message

#### Scenario: Not spam after all
- **WHEN** the message Marta's `reply` need comes from is moved to spam and later back to the inbox
- **THEN** the need is cleared as `trashed`, then open again from the same message

#### Scenario: A trash round trip
- **WHEN** a confirmed need's message and a Done need's message are trashed and then restored
- **THEN** the confirmed need is open with basis `member`, and the Done one stays cleared as `done`

#### Scenario: A move and a refresh at once
- **WHEN** a decide pass refreshes a conversation while a provider move of its message to spam is not yet committed
- **THEN** the refresh waits for the move and stores no need for that message
