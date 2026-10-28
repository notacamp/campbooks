## ADDED Requirements

### Requirement: The sender's sentence under Why it's here
For a task made from an email, the detail SHALL show, under the sender's line of
the email card in "Why it's here", the sender's own sentence that asked for it
as a quotation (`<blockquote>`, no language guessed), in quotation marks by the
member's language (“…” en; « … » fr; «…» pt-PT and es-ES). It SHALL come from
the conversation's read of the message, which the server answers only to members
who may read the email, joined by the task's `source_ask`. It SHALL show nothing
for a task with no such ask, while loading, or when the caller may not read the
email, including a task handed to them from an inbox they cannot read.

#### Scenario: The lease
- **WHEN** Ana opens the task Scout made from Marta's email
- **THEN** Marta's sentence "Could you send the signed lease back to me by Friday?" shows quoted under her name

#### Scenario: An older task
- **WHEN** Ana opens a task made before the sentence could be served
- **THEN** no quotation shows

#### Scenario: A teammate without access
- **WHEN** a member who cannot read the inbox opens a task handed to them
- **THEN** no quotation shows
