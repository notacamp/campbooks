## ADDED Requirements

### Requirement: The member's reply is a fact for other modules
`conversations` SHALL publish `conversations.member_replied` v1 in the
transaction that stores an outbound message for the first time in a conversation
of a personal inbox, whether the member sent it from Campbooks or from the
provider's own client: the workspace, inbox, conversation, message, the time it
was sent and the inbox's owner. It SHALL hold no name, address, subject or text.
It SHALL NOT be published for a shared inbox, for mail read by an inbox's first
sync, or again when a message is stored again.

#### Scenario: Sam answers from his phone
- **WHEN** a change batch brings a message Sam sent from his mail app into his personal inbox
- **THEN** one `member_replied` names the conversation, the message, its time and Sam, and no address

#### Scenario: A shared inbox
- **WHEN** a message sent from an inbox several members read is stored
- **THEN** no `member_replied` is published

#### Scenario: The batch again
- **WHEN** the same batch is applied again
- **THEN** no second `member_replied` is published
