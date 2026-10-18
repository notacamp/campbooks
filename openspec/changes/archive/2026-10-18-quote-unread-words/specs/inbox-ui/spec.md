## ADDED Requirements

### Requirement: Quoting mail whose words came later
When `conversations.get` marks a message `text_unread`, the client SHALL ask
for its words alone (`words=true` on its body), once per message, before
quoting it in a reply, reply all or forward (the composer, the reply dialog
and Home's Forward picker), and for its folded line in the reader. While
they are read, a neutral line SHALL say so. When they cannot be read,
nothing SHALL be started or sent and the member SHALL be told why: a reply
or forward SHALL never carry an empty quote silently.

#### Scenario: Forwarding a message whose words came later
- **WHEN** a member forwards a message marked `text_unread`
- **THEN** "Reading the email to quote it…" shows, and the forward's quote carries the message's words

#### Scenario: Its words cannot be read
- **WHEN** a member forwards a message marked `text_unread` and its words cannot be read
- **THEN** no draft is started and "Couldn't read the email to quote it" says why

#### Scenario: A folded message whose words came later
- **WHEN** the reader folds a message marked `text_unread`
- **THEN** its line says "Reading…" until its words are read, then shows them
