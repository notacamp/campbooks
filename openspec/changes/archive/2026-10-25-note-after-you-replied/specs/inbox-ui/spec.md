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
(1), which the user can undo. The note SHALL offer Reply, opening the same
reply dialog as the reader's own Reply, whenever it says a reply is needed (a
needs_reply suggestion or an open Reply need) and the user can reply. The
decide pass's answers are about the conversation's newest inbound message:
when the conversation has no open need and the user sent a message after
that one, the note SHALL say they replied and when ("You replied on 23 Oct,
so nothing here waits on you."), SHALL NOT suggest a reply, an ask, a date
or urgency from those answers, and SHALL offer no Reply; a suspicious
warning SHALL still be said.

#### Scenario: Taking a suggestion
- **WHEN** the user opens a newsletter that the rules suggest for Newsletters and presses 1
- **THEN** it moves to Newsletters, the toast offers Undo, and "How Campbooks decided" had listed the mailing-list header reason

#### Scenario: Replying from the note
- **WHEN** the user opens a conversation whose note says it needs a reply
- **THEN** the note offers Reply, which opens Scout's reply dialog

#### Scenario: No numbers up front
- **WHEN** the user opens a conversation whose stream answer has a confidence of 0.87
- **THEN** the note shows no percentage, and 0.87 appears only inside "How Campbooks decided"

#### Scenario: After the user replied
- **WHEN** the user opens a conversation with no open need whose newest inbound message the decide pass read as needing a reply, and they sent a reply on 23 Oct after it
- **THEN** the note says "You replied on 23 Oct, so nothing here waits on you.", does not say it needs a reply, and offers no Reply

#### Scenario: A warning stands after a reply
- **WHEN** the user replied after a message the decide pass found suspicious
- **THEN** the note says they replied and still says the message looks suspicious
