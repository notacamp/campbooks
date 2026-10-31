## ADDED Requirements

### Requirement: From, mode and subject in the composer
When the member may send from more than one inbox, the composer SHALL show
From as a picker of those inboxes, for every kind of draft. The composer's
title SHALL switch between Reply, Reply all and Forward, keeping the text.
The subject SHALL be shown and editable for every kind.

#### Scenario: Switch to Reply all
- **WHEN** the user has written two lines in a reply and switches the title to Reply all
- **THEN** the composer shows the reply-all recipients and the same two lines

#### Scenario: One inbox
- **WHEN** the member sends from one inbox only
- **THEN** From shows that inbox's address, with no picker

### Requirement: Scout in the composer
On a reply, reply-all or nudge draft, and while Scout can write replies,
the composer SHALL offer Scout ▾ (⌘J) with Shorter, Warmer, Fix spelling and
Change…. Each SHALL rewrite the text in place, show Scout's thinking while
it works, and offer Undo. Scout's text SHALL go in as plain text, with no
formatting and no link whose words hide where it goes.

#### Scenario: Shorter
- **WHEN** the user picks Scout ▾ → Shorter on a reply
- **THEN** the text is replaced by Scout's shorter version, and Undo puts the previous text back

#### Scenario: Scout writes a worded link
- **WHEN** Scout's rewrite contains `[pay here](https://example.net)`
- **THEN** the composer shows those characters as text, with no link
