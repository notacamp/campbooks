## MODIFIED Requirements

### Requirement: Reading a conversation
The reader SHALL show the conversation's subject, the note, and its messages
newest first with the newest (and any unread) open and older ones folded to
one line until opened, each with sender, recipients, time and attachments.
A message the inbox sent SHALL read "You", then "→" and its first recipient
(and how many others), set in from the left on the sunken surface (further
from 48rem); everyone else's messages SHALL stay flush left. In a thread of
more than four messages the reader SHALL keep the newest two, the first,
every unread one and every one the member opened, and fold each run of two
or more of the others into one "N more messages" button that shows them, as
folded lines, in its place until another conversation opens. A message's
trailing quoted history SHALL be folded behind "Show quoted text" ("Hide
quoted text" once shown): in an HTML body the block the server marked, as a
`details` element that opens with no script, the frame growing to fit; in a
plain-text body the trailing run of lines that start with `>`, when the
message has words of its own before it. Nothing about the words decides what
is quoted.
HTML bodies SHALL be shown only in a sandboxed frame that never runs scripts
(sandbox exactly `allow-same-origin allow-popups
allow-popups-to-escape-sandbox`: no `allow-scripts`, no forms, no top
navigation), with its own Content-Security-Policy and no referrer. The frame
SHALL be as tall as its content (measured, and again when its width or
content changes; never scrolling inside except sideways), SHALL shrink a
layout wider than the reader to fit, down to half size, and SHALL apply the
message's filtered `@media` rules to its own document only, never to the
app's page. Remote images SHALL stay blocked until the user chooses "Show
images" for that message (Shift+I for all). Opening an unread conversation
SHALL mark it read.

#### Scenario: A newsletter with remote images
- **WHEN** the user opens a message whose body has a remote image
- **THEN** the frame's policy allows only `data:` images, the reader says one remote image is hidden, and after "Show images" the frame allows `https` images

#### Scenario: A long message
- **WHEN** the user opens a message whose content is 3,000px tall
- **THEN** the frame is 3,000px tall, cut nowhere and not padded, and the reader scrolls, not the frame

#### Scenario: A fixed 600px layout on a phone
- **WHEN** the user opens a message laid out at 600px wide at 375px
- **THEN** it is shown shrunk to the frame's width and the page does not scroll sideways

#### Scenario: A responsive newsletter on a phone
- **WHEN** a message's `@media (max-width: 620px)` rule makes its container full width and the frame is 343px wide
- **THEN** the rule applies inside the frame, the layout fits without shrinking, and the app's own page is unaffected

#### Scenario: A long back and forth
- **WHEN** the member opens a thread of seven read messages, three of them sent by Sam
- **THEN** the newest two and the first show, "4 more messages" stands between them, Sam's read "You → Marta Silva" set in from the left, and pressing it shows the four as folded lines

#### Scenario: A reply with the whole thread quoted
- **WHEN** the member opens Marta's Gmail reply whose body ends with the earlier messages in a `gmail_quote`
- **THEN** only her new words show, with "Show quoted text" under them, and opening it shows the quote in the frame

#### Scenario: An inline reply is not folded
- **WHEN** a plain-text reply answers between the lines it quotes
- **THEN** the whole message shows and there is no "Show quoted text"

### Requirement: The reply slot
The reader SHALL expose a slot for the send slice's composer, pinned to the
bottom of the reader after the thread and never between or after its
messages in the flow, with Reply (R), Reply all (Shift+R) and Forward (F) that
tell the slot which message to answer and how; without a composer those
actions SHALL be hidden. From 48rem the pinned area SHALL stay in view while
the thread scrolls under it, showing "Reply to <name>" (R) while the composer
is closed and the open composer in its place, growing upward to 60% of the
window's height and then scrolling inside itself. Below 48rem the floating
bar above the tab bar SHALL offer Reply and Done, and an open composer SHALL
take its place. Reply SHALL first open the reply dialog (Scout's likely replies and
draft); its "Write my own" and "Open in composer" SHALL open the slot's
composer on the newest message, the latter on the draft Scout wrote. Reply
all and Forward SHALL open the composer directly.

#### Scenario: No composer yet
- **WHEN** the inbox runs without a reply-slot renderer
- **THEN** the reader shows no Reply, Reply all or Forward

#### Scenario: Write my own
- **WHEN** the user presses R in the reader, then chooses "Write my own"
- **THEN** the dialog closes and the composer opens in the pinned area at the bottom of the reader

#### Scenario: A long thread on a desktop
- **WHEN** the member opens a twelve-message thread at 1440px and scrolls it
- **THEN** "Reply to Marta Silva" stays at the bottom of the reader and no reply box sits between or after the messages

## ADDED Requirements

### Requirement: Who wrote last
A conversation row whose newest message the inbox sent SHALL start its
snippet with "You:" ("Eu:", "Yo:", "Moi :" in pt-PT, es-ES and fr), read
from the list's `latest_from_you`.

#### Scenario: Waiting on them
- **WHEN** Sam's reply is the newest message of a conversation in Primary
- **THEN** its row's snippet reads "You: Revised quote attached…"
