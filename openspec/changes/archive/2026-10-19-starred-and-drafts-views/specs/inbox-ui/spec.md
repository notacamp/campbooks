## MODIFIED Requirements

### Requirement: Streams, not people
The inbox SHALL be its streams: Primary, Updates, Receipts and Newsletters,
listed under Inbox in the sidebar (chips on phones) with their unread counts,
opened with G then 1 to 4. A stream's page SHALL list its pinned
conversations first, then the rest newest first, a page at a time, with
sender, time, subject, snippet, unread, pinned and message count, and All or
Unread. Six places SHALL sit under the streams, after a quiet divider, in this
order: Sent, Pinned, Drafts, Archive, Spam and Trash, each with its icon and
no count. Sent, Archive, Spam and Trash SHALL open with G then 5 to 8, Pinned
with G then 9 and Drafts with G then 0: the keys follow history, the order
follows meaning. On phones they SHALL follow the stream chips, after a
divider, in the same row, which scrolls sideways on its own; the page never
does. Sent, Pinned, Archive, Spam and Trash are views; Drafts lists your
saved drafts and not conversations. None is a stream: nothing is sorted into
them, and a conversation can be in a stream and a view at once. Pinned SHALL
list the conversations the user pinned, in any stream, archived or not,
outside the trash, newest first; pinning or unpinning SHALL add it to or take
it out of the open Pinned list at once. Any other place in the address SHALL
open Primary.

#### Scenario: Open a stream from the keyboard
- **WHEN** the user presses G then 3
- **THEN** the Receipts stream opens

#### Scenario: A pinned conversation
- **WHEN** the user pins an older conversation in Primary
- **THEN** it moves to the top of Primary, marked pinned, and stays there as newer mail arrives

#### Scenario: Open a view from the keyboard
- **WHEN** the user presses G then 8
- **THEN** Trash opens, marked as the current place in the sidebar

#### Scenario: Open Pinned and Drafts from the keyboard
- **WHEN** the user presses G then 9, then G then 0
- **THEN** Pinned opens, then Drafts, each marked as the current place in the sidebar, and G then 5 to 8 still open Sent, Archive, Spam and Trash

#### Scenario: Pinned across streams
- **WHEN** the user has pinned a conversation in Primary and an archived one in Receipts
- **THEN** Pinned lists both, newest first, and unpinning one takes it out of the list at once

#### Scenario: Views on a phone
- **WHEN** the inbox is open at 375px
- **THEN** the chips show the four streams, a divider, then Sent, Pinned, Drafts, Archive, Spam and Trash, and only the chip row scrolls sideways

#### Scenario: An empty view
- **WHEN** Spam has nothing in it
- **THEN** it says "Nothing in Spam." in one line

## ADDED Requirements

### Requirement: Drafts
Drafts (G then 0, `/inbox/drafts`) SHALL list the user's own saved drafts from
`conversations.list_drafts`, newest first, at most 50, without paging. Each
row SHALL say "Draft" and whom it is to ("No recipients" when none), show the
subject ("(no subject)" when empty), a line of its words and when it was
saved. Drafts already scheduled or sending SHALL not be listed. Opening a row
SHALL open the composer where the draft was left: a new message on its own
screen, an answer under its thread, on the message it answers. Each row SHALL
have Delete draft, with a toast and Undo that writes the same draft again.
The list SHALL refresh when a draft is saved, sent or deleted. With none it
SHALL say "No drafts." in one line, and when exactly 50 are listed it SHALL
add "Your 50 newest drafts."

#### Scenario: Open a draft
- **WHEN** the user opens a saved new message in Drafts
- **THEN** the composer opens on that draft, with its recipients, subject and text

#### Scenario: Open a reply draft
- **WHEN** the user opens a saved reply in Drafts
- **THEN** its conversation opens with the reply composer on that draft

#### Scenario: Delete and Undo
- **WHEN** the user deletes a draft from the list, then presses Undo in the toast
- **THEN** it leaves the list with "Draft deleted", and Undo puts the same draft back

#### Scenario: Nothing saved
- **WHEN** the user has no drafts
- **THEN** Drafts says "No drafts." in one line

#### Scenario: The newest 50
- **WHEN** the user has 50 or more drafts
- **THEN** Drafts lists the newest 50 and notes "Your 50 newest drafts."

### Requirement: The active place's chip is in view on phones
When the place changes on a phone, the chip of the stream or view now open
SHALL scroll into view in the chip row, the nearest edge, without moving the
page sideways. The scroll SHALL animate, except under `prefers-reduced-motion`,
where it SHALL jump.

#### Scenario: Trash on a phone
- **WHEN** the user taps Trash, the last chip, at 375px
- **THEN** the chip row scrolls so the Trash chip is fully visible

#### Scenario: Reduced motion
- **WHEN** the user prefers reduced motion and opens Drafts
- **THEN** its chip is brought into view without animation
