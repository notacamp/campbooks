## ADDED Requirements

### Requirement: Explain this thread in the reader
While the conversation detail says the member may ask, the reader's "Where
things stand" note SHALL offer "Explain this thread" (with its shortcut and
hint, and in ⌘K).

**The card** below the note SHALL show:
- while writing: "Scout is reading the N messages…" with a skeleton;
- then In short, Who wants what, Agreed, Still open and On you (emphasised),
  each point with chips naming the sender and date of the messages it comes
  from, which open them;
- "From N messages · written <time>", saying when the older messages came
  from Scout's notes;
- "Ask a follow-up", which opens the Scout chat about the thread;
- "Only you see this."

The card SHALL render its texts as plain text, with no links or markup. When
the explanation is stale, the card SHALL say which newer message it predates
and offer "Explain again". It SHALL say plainly when the daily AI limit was
reached, keeping any earlier explanation, and when Scout couldn't explain
the thread, offering to try again.

On a phone the card SHALL show In short and On you first, with the rest one
tap away. Every string SHALL be in en, pt-PT, es-ES and fr.

#### Scenario: Asking
- **WHEN** Ana presses the explain shortcut on "Kickoff agenda"
- **THEN** the card shows Scout reading the nine messages, then the explanation with "On you: send Jonas the revised quote before Thu 6 Nov"

#### Scenario: A chip
- **WHEN** Ana taps "Jonas · 29 Oct" beside a point
- **THEN** the reader scrolls to that message and opens it

#### Scenario: Not offered
- **WHEN** Scout can't read mail on this server, or the thread raised a phishing warning
- **THEN** the note has no "Explain this thread"
