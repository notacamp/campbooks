## ADDED Requirements

### Requirement: A chat about a thread knows the member's explanation
In a chat about a conversation, what the chat is about SHALL also carry the
member's own explanation of it, when they have one they may still see
(within 3 KiB, with whether it is stale and when it was written), read as the
member through `conversations.get_explanation`. Scout SHALL treat it as its
own earlier reading, to be checked against the messages when stale.

#### Scenario: A follow-up
- **WHEN** Ana chooses "Ask a follow-up" under her explanation and asks "what price did we give for the first workshop?"
- **THEN** Scout answers with the explanation and the messages in view, and says if the explanation is out of date
