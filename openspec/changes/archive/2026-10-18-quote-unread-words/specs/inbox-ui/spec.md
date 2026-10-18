## ADDED Requirements

### Requirement: Quoting mail whose words came later
When `conversations.get` marks a message `text_unread`, the client SHALL ask
for its words alone (`words=true` on its body), once per message, before
quoting it in a reply, reply all or forward (the composer, the reply dialog
and Home's Forward picker), and for its folded line in the reader. While
they are read, a neutral line SHALL say so. When they cannot be read,
nothing SHALL be started or sent and the member SHALL be told why: a reply
or forward SHALL never carry an empty quote silently, nor a quote the member
was not shown. In the reply dialog, Send, its shortcut and "Open in composer"
SHALL wait until the quote is shown, and "Try again" SHALL read the words
again and show them. When the words came `truncated`, the composer SHALL say
the quote holds the email's first part. A folded line whose words cannot be
read SHALL say so rather than stay blank.

#### Scenario: Forwarding a message whose words came later
- **WHEN** a member forwards a message marked `text_unread`
- **THEN** "Reading the email to quote it…" shows, and the forward's quote carries the message's words

#### Scenario: Its words cannot be read
- **WHEN** a member forwards a message marked `text_unread` and its words cannot be read
- **THEN** no draft is started and "Couldn't read the email to quote it" says why

#### Scenario: Replying after the quote could not be read
- **WHEN** the reply dialog could not read the words of the message it answers
- **THEN** Send is disabled and says why, and after "Try again" reads them the quote shows and Send may go

#### Scenario: A long email quoted
- **WHEN** a member forwards a message whose words came `truncated`
- **THEN** the composer says "This email is long: the quote holds its first part."

#### Scenario: A folded message whose words came later
- **WHEN** the reader folds a message marked `text_unread`
- **THEN** its line says "Reading…" until its words are read, then shows them
