## ADDED Requirements

### Requirement: Room to write
The message composer (Reply, Reply all, Forward, New message, and a draft
reopened from Drafts) SHALL set From, To, Cc, Bcc and the subject at the
body text size, in rows of at least 40px. Its text area SHALL be at least
ten lines at the body size and grow with the text. Its footer, holding Send,
Delete draft and the save state, SHALL stay at the bottom of the composer
while the text scrolls.

#### Scenario: Long text in the reader
- **WHEN** the user writes 40 lines in a reply in the reader
- **THEN** the composer grows to 60% of the window, the text scrolls inside it, and Send stays in view

### Requirement: Formatting shows as formatting
The composer's text SHALL show bold, italic, strikethrough, links with
words, bulleted and numbered lists and quotes as formatting, never as the
marks it is saved as, with undo and redo on ⌘Z and ⇧⌘Z. It SHALL save to
and open from those marks (conversations, "The marks a message may carry"),
so a reopened draft looks as it was left. Pasted content SHALL keep its text
and only those kinds of formatting.

#### Scenario: Bold shows as bold
- **WHEN** the user selects "renew for a year" and presses ⌘B
- **THEN** the words show in bold, with no asterisks, and the draft saves `**renew for a year**`

#### Scenario: A draft reopens as it was left
- **WHEN** the user reopens a draft saved as two bullet lines, `- parking P2-14` and `- rent on the ~~5th~~ 1st`
- **THEN** the composer shows a bulleted list of two items, with "5th" struck through

#### Scenario: Typing an asterisk
- **WHEN** the user types `2 * 3 = 6`
- **THEN** it shows as typed, and the draft saves `2 \* 3 = 6`

#### Scenario: Pasting from a web page
- **WHEN** the user pastes a heading, an image and a bold sentence
- **THEN** the heading's text becomes a paragraph, the image is dropped, and the sentence stays bold

### Requirement: The formatting toolbar
The composer SHALL offer Bold (⌘B), Italic (⌘I), Strikethrough (⇧⌘X), Link
(⇧⌘K), Bullets (⇧⌘8), Numbers (⇧⌘7) and Quote (⇧⌘9), each with a tooltip
naming its key. The toolbar SHALL stay one row: in a composer narrower than
32rem, Strikethrough, Numbers and Quote SHALL fold into a More menu, and
their keys work either way.

#### Scenario: A phone
- **WHEN** the composer is open at 375px
- **THEN** its toolbar is one row with Bold, Italic, Link, Bullets and More, and nothing overflows sideways

### Requirement: Links in the composer
Link SHALL ask for the words and the address and SHALL take only `http:`,
`https:` and `mailto:` addresses. Pasting an address over selected words
SHALL link them.

#### Scenario: Link words to an address
- **WHEN** the user selects "the lease", presses ⇧⌘K and enters `https://example.com/4b`
- **THEN** "the lease" shows as a link, and the draft saves `[the lease](https://example.com/4b)`

#### Scenario: A link that is not allowed
- **WHEN** the user enters `javascript:alert(1)` as a link's address
- **THEN** no link is made, and the dialog says only web and mail addresses can be linked

### Requirement: Expanding the composer
From 48rem, Expand (⇧⌘E) SHALL open the same draft, with its cursor and undo
history, in a sheet about 820px wide over the page, and Shrink or Esc SHALL
put it back where it was. Below 48rem Expand SHALL be hidden.

#### Scenario: Expand and back
- **WHEN** the user presses ⇧⌘E while writing, types a sentence, and presses Esc
- **THEN** the sheet opens on the same text with the cursor where it was, the sentence is kept, and the composer is back in the reader

#### Scenario: No Expand on a phone
- **WHEN** the composer is open at 375px
- **THEN** Expand is not shown
