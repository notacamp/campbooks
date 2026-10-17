## MODIFIED Requirements

### Requirement: Inboxes from connections
When `connectors.capability_granted` is published for the `messages`
capability, the module SHALL create the inbox it names, with the address and
display name of the connection-account document it points at, and make the
granting member its `manager`. When that adds the member to the inbox, it
SHALL publish `conversations.inbox_changed` in the same transaction, so
open change streams list the member's inboxes again before reading on.
Other capabilities SHALL create nothing, and a repeated event SHALL change
nothing and publish nothing. When `connectors.messages_changed` is
published, the module SHALL enqueue the application of its batch, ordered
per inbox.

#### Scenario: A Gmail connection
- **WHEN** a `messages` grant names an inbox, then a batch is announced for it
- **THEN** the member sees the inbox as its manager, and the batch's conversations in it

#### Scenario: A calendar grant
- **WHEN** a `calendar` grant is published
- **THEN** no inbox is created

#### Scenario: The member is added
- **WHEN** a `messages` grant creates an inbox for a member, and the same grant is published again
- **THEN** `conversations.inbox_changed` is published once, for that inbox
