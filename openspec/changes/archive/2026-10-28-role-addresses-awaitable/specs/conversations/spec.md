## MODIFIED Requirements

### Requirement: The people behind a question
A need that is a question about a reply or an action (`review`, trust
`check`, question `needs_reply` or `asks_action`) on a message that went to
people besides the inbox SHALL carry `unsure` in its summary: those people
(`To` before `Cc`, as the headers gave them, without the inbox, the sender and
addresses that cannot answer, at most six; a role address stays), each named
by its display name (header name, else organisation, else address), whether the inbox is in `To` and whether it is in `Cc`, the
one natural party (the person the model's `awaited` answer named, when its
stored option still names that address, else the only other person in `To`;
none when neither exists), the days since the message when none of those
people wrote to the inbox since (none when one did), and the model's
confidence and the sure bar of the question in whole percent. It SHALL be
worked out from the headers, the stored answers and the inbox's messages
only, never from what a message says. A message that went to nobody else
SHALL carry none. The person a `waiting` or `nudge` need waits on SHALL be
named the same way.

#### Scenario: Copied on mail between others
- **WHEN** João writes to Ana Costa, copying the inbox, and the model answers `needs_reply` true at 0.97
- **THEN** the need is a `check` question whose `unsure` lists Ana Costa, says the inbox is not in `To`, names her as the party, and carries confidence 97 and sure bar 90

#### Scenario: Written to the inbox alone
- **WHEN** Marta writes to the inbox alone and the model answers `needs_reply` true at 0.85
- **THEN** the need is a `check` question with no `unsure`

#### Scenario: She wrote back
- **WHEN** Ana Costa wrote to the inbox after João's message
- **THEN** `unsure` carries no days unanswered

#### Scenario: A shared mailbox
- **WHEN** Sam wrote to "Geral" <geral@aguas-ribatejo.example>
- **THEN** `unsure` lists it as "Aguas Ribatejo"
