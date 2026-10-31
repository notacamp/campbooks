# settings-ui Specification

## Purpose
The layout every Settings and Admin page shares: a title and lead, then
sections as cards, so no page reads as one undifferentiated column.

## Requirements

### Requirement: Settings pages are made of section cards
Every Settings and Admin page SHALL show its title in the display face at the
page-title size with its lead beneath, then its sections as cards: a raised
surface with a subtle border, a title at 17px semibold, an optional count shown
as "Title · N" in the tertiary tone, an optional one-line description at 13px
tertiary, and the section's content. A list inside a card SHALL drop its own
panel and divide its rows by lines; a fact list SHALL sit on a sunken panel
instead of a second card. A section with nothing to show SHALL say so in a quiet
sunken panel with a bold first line and guidance, not a page-sized empty state.
A page's actions SHALL sit beside its title and wrap under it on phones. The
layout SHALL use design tokens only, work in light and dark, and not scroll
horizontally at 375px.

#### Scenario: Sections are told apart
- **WHEN** a member opens any Settings page
- **THEN** each section is its own card whose title is larger than body text and than a field label

#### Scenario: A phone
- **WHEN** a page is opened at 375px
- **THEN** the cards stack in one column and nothing scrolls sideways

### Requirement: Your signature in Settings
Settings → Mailboxes SHALL give each mailbox the member can send from a
"Your signature" editor with bold, italic and links. It SHALL save with
Save or ⌘↵, confirm with "Signature saved", and say when the text is too
long.

#### Scenario: Save a signature
- **WHEN** the user writes "Sam Rivers" with "Rivers Lettings" in bold and presses Save
- **THEN** "Signature saved" shows, and the next reply from that mailbox carries it
