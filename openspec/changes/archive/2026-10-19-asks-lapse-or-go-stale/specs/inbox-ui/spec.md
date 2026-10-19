## ADDED Requirements

### Requirement: An expired ask
A conversation row whose summary carries `lapsed_need` and no `need` SHALL
show the chip "Expired" in a neutral tone, and the reader's note SHALL show
the same chip, with no Done, Not needed, Yes or No for it. A still-needed
question SHALL be asked in the reader as on Home ("Pedro Alves asked 3 weeks
ago. Still to do?") with Yes (1) and No (2), under the "Check" chip.

#### Scenario: A sign-in link
- **WHEN** the user opens the inbox after a sign-in link's ten minutes have passed
- **THEN** its row shows "Expired" instead of "To do", and its reader offers nothing to resolve

#### Scenario: An old ask in the reader
- **WHEN** the user opens Pedro Alves's three-week-old request
- **THEN** the note asks "Pedro Alves asked 3 weeks ago. Still to do?" with Yes and No
