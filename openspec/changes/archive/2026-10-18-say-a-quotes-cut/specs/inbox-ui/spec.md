## ADDED Requirements

### Requirement: Saying what a quote leaves out
The composer SHALL say "the quote holds its first part" when the message it
quotes is `quote_cut` or its text or words came `truncated`. The reply
dialog's preview SHALL say when the reply quotes only the email's first part,
and otherwise, when the preview shows only the start of what the reply
quotes, that it is shown in part (the preview ending in "…").

#### Scenario: Replying to a long email
- **WHEN** a member opens Reply on a message marked `quote_cut`
- **THEN** the dialog's quote says "This email is long: your reply quotes its first part.", and Forward's composer says "This email is long: the quote holds its first part."

#### Scenario: A long quote in a short preview
- **WHEN** the reply dialog answers a message whose words are longer than its preview
- **THEN** the preview ends in "…" and says "Shown in part: your reply quotes all of it."
