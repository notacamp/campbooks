## ADDED Requirements

### Requirement: Needs on rows and in the reader
A conversation row with an open need SHALL show a chip for it ("Needs a
reply", "To do", "Pay", "Looks suspicious", or "Check" for a question) and,
when it has one, its due date ("Due Thu", "Overdue"). The reader's note SHALL
lead with the need's sentence, in the user's language, naming the sender (by
address for a `review` need), then the sender's sentence quoted as plain text
with who wrote it and when, then the chips, then Done (E) and Not needed (X);
a `check` need SHALL ask its question with Yes (1) and No (2). Every action
SHALL push an entry Undo (Z) takes back. The quote SHALL never be rendered as
HTML or turned into links.

#### Scenario: A question with a date
- **WHEN** the user opens Marta's conversation whose `reply` need is due Wednesday
- **THEN** the note says "Marta Ferreira is waiting for your reply", quotes "Could you confirm Thursday's walkthrough? The builders need to know by Wednesday.", shows "Needs a reply" and "Due Wed", and E clears it with Undo in the toast

#### Scenario: Phishing
- **WHEN** the user opens a conversation with a `review` need for suspicious mail
- **THEN** the note says "This may be phishing" with the sender's address and its reasons before the quote, and offers "It's safe"

## MODIFIED Requirements

### Requirement: Suggestions shown as suggestions
The decide pass's answers SHALL be shown as suggestions, never applied by the
client on its own: a row whose conversation has a stream suggestion SHALL say
"Suggests <stream>"; the reader's note SHALL say what the decide pass thinks
in plain words, as suggestions, and who placed the conversation, with no
percentage. "How Campbooks decided" (closed by default) SHALL list, for each
question, the answer in words, the stored reasons in the user's language, the
source (rules or model), the confidence and the threshold the answer was held
to, and whether it acted. A stream suggestion SHALL offer "Move to <stream>"
(1), which the user can undo.

#### Scenario: Taking a suggestion
- **WHEN** the user opens a newsletter that the rules suggest for Newsletters and presses 1
- **THEN** it moves to Newsletters, the toast offers Undo, and "How Campbooks decided" had listed the mailing-list header reason

#### Scenario: No numbers up front
- **WHEN** the user opens a conversation whose stream answer has a confidence of 0.87
- **THEN** the note shows no percentage, and 0.87 appears only inside "How Campbooks decided"
