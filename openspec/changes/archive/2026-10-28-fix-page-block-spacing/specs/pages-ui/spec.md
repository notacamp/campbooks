## ADDED Requirements

### Requirement: Blocks are spaced to read
A page's blocks SHALL follow one spacing rhythm, at the top of the page and
inside callouts, quotes, toggles and list items: a small gap between blocks,
a larger one around boxed blocks (callouts, quotes, code, cards, images,
tables), more space above a heading than below it, and no extra space at the
top or bottom edge of the page or of a box. Bullet and numbered lists SHALL
show a marker for each item. A closed toggle SHALL show only its first line.
A to-do's checkbox SHALL sit on the first line of its text, and on a touch
screen each to-do SHALL be at least a finger tall.

#### Scenario: A heading and its text
- **WHEN** a page has a paragraph, then a heading, then a paragraph
- **THEN** the heading is further from the paragraph above it than from the paragraph under it, and neither paragraph touches it

#### Scenario: Two callouts in a row
- **WHEN** a page has two callouts one after the other
- **THEN** there is visible space between them, and the text inside each starts at its padding

#### Scenario: A closed toggle
- **WHEN** a toggle with two lines is closed
- **THEN** only its first line shows
