# inbox-ui Specification

## Purpose
The inbox screens of the app: a workspace's streams, each stream's
conversations, and the reader with the thread, the assistant's suggestions
and their reasons, safe email bodies, tidying with undo, and the slot the
send slice's composer fills.

## Requirements

### Requirement: Streams, not people
The inbox SHALL be its streams: Primary, Updates, Receipts and Newsletters,
listed under Inbox in the sidebar (chips on phones) with their unread counts,
opened with G then 1 to 4. A stream's page SHALL list its conversations
newest first, a page at a time, with sender, time, subject, snippet, unread,
starred and message count, and All or Unread.

#### Scenario: Open a stream from the keyboard
- **WHEN** the user presses G then 3
- **THEN** the Receipts stream opens

### Requirement: Suggestions shown as suggestions
The decide pass's answers SHALL be shown as suggestions, never applied by the
client on its own: a row whose conversation has a stream suggestion SHALL say
"Suggests <stream>"; the reader's note SHALL say what the decide pass thinks
in plain words, as suggestions, and who placed the conversation, with no
percentage. "How Campbooks decided" (closed by default) SHALL list, for each
question, the answer in words, the stored reasons in the user's language, the
source (rules or model), the confidence and the threshold the answer was held
to, and whether it acted. A stream suggestion SHALL offer "Move to <stream>"
(1), which the user can undo.

#### Scenario: Taking a suggestion
- **WHEN** the user opens a newsletter that the rules suggest for Newsletters and presses 1
- **THEN** it moves to Newsletters, the toast offers Undo, and "How Campbooks decided" had listed the mailing-list header reason

#### Scenario: No numbers up front
- **WHEN** the user opens a conversation whose stream answer has a confidence of 0.87
- **THEN** the note shows no percentage, and 0.87 appears only inside "How Campbooks decided"

### Requirement: Reading a conversation
The reader SHALL show the conversation's subject, the note, and its messages
newest first with the newest (and any unread) open and older ones folded to
one line until opened, each with sender, recipients, time and attachments.
HTML bodies SHALL be shown only in a sandboxed frame (no scripts, an opaque
origin, its own Content-Security-Policy, no referrer); remote images SHALL
stay blocked until the user chooses "Show images" for that message (Shift+I
for all). Opening an unread conversation SHALL mark it read.

#### Scenario: A newsletter with remote images
- **WHEN** the user opens a message whose body has a remote image
- **THEN** the frame's policy allows only `data:` images, the reader says one remote image is hidden, and after "Show images" the frame allows `https` images

### Requirement: Tidying with undo
Done (E, archive), star (S), read or unread (U) and move to a stream (M, then
1 to 4) SHALL act on the open conversation, or on the list's cursor (J/K),
show at once, and push an entry that Undo (Z, or the toast) takes back. After
Done the next conversation opens (the list, on phones).

#### Scenario: Done and Undo
- **WHEN** the user presses E on an open conversation, then Z
- **THEN** the conversation is archived, the toast says "Done · <subject>", and Z unarchives it

### Requirement: The reply slot
The reader SHALL expose a slot at the end of the thread for the send slice's
composer, with Reply (R), Reply all (Shift+R) and Forward (F) that tell the slot
which message to answer and how; without a composer those actions SHALL be
hidden.

#### Scenario: No composer yet
- **WHEN** the inbox runs without a reply-slot renderer
- **THEN** the reader shows no Reply, Reply all or Forward

### Requirement: Separate screens on phones
From 64rem the list and the reader SHALL show side by side; below it they
SHALL be separate screens, the reader with "Back to <stream>" (Esc), and
below 48rem the reader's Done (and Reply) SHALL float above the tab bar.
Every screen SHALL exist in en, pt-PT, es-ES and fr, show loading, empty and
error states (including no mailbox and a conversation that is gone), fit
375px without horizontal scrolling and pass axe in light and dark.

#### Scenario: Opening a conversation on a phone
- **WHEN** the user taps a conversation at 375px
- **THEN** the reader replaces the list, and Back returns to the stream

### Requirement: Attachments
Each open message SHALL list its attachments, except inline images shown in
its body, with a type icon, the file name (bidirectional and control
characters removed), the size, a Download action and, for raster images and
PDFs, a Preview action. An image preview SHALL show the picture as an image
element of a `blob:` URL whose type the client sets from its raster list,
in a dialog with Download and Close; a PDF preview SHALL open the
attachment in a new tab, served inline under a sandbox policy. HTML, SVG and
every other type SHALL be download-only. V SHALL move focus through the
open message's attachments, wrapping, and Enter SHALL open the focused one;
the list SHALL show the V hint.

#### Scenario: Preview a photo
- **WHEN** the member presses V on a message with a JPEG attachment, then Enter
- **THEN** a dialog shows the photo with its name, Download and Close, and Escape closes it

#### Scenario: An HTML attachment
- **WHEN** a message has an `invoice.html` attachment
- **THEN** its row offers Download only

### Requirement: Placeholder for blocked images
A remote image held back in an email body SHALL show the design system's
placeholder at its declared size instead of the browser's broken-image
icon, and SHALL load nothing until the member chooses "Show images".

#### Scenario: A newsletter banner
- **WHEN** the member opens a newsletter with a remote banner and has not shown images
- **THEN** the banner's place shows the placeholder, and no request leaves for the banner's host

### Requirement: Needs on rows and in the reader
A conversation row with an open need SHALL show a chip for it ("Needs a
reply", "To do", "Pay", "Looks suspicious", or "Check" for a question) and,
when it has one, its due date ("Due Thu", "Overdue"). The reader's note SHALL
lead with the need's sentence, in the user's language, naming the sender (by
address for a `review` need), then the sender's sentence quoted as plain text
with who wrote it and when, then the chips, then Done and Not needed (X)
(E stays the inbox's Done, which archives); a `check` need SHALL ask its
question with Yes (1) and No (2). Every action
SHALL push an entry Undo (Z) takes back. The quote SHALL never be rendered as
HTML or turned into links.

#### Scenario: A question with a date
- **WHEN** the user opens Marta's conversation whose `reply` need is due Wednesday
- **THEN** the note says "Marta Ferreira is waiting for your reply", quotes "Could you confirm Thursday's walkthrough? The builders need to know by Wednesday.", shows "Needs a reply" and "Due Wed", and its Done clears it with Undo in the toast

#### Scenario: Phishing
- **WHEN** the user opens a conversation with a `review` need for suspicious mail
- **THEN** the note says "This may be phishing" with the sender's address and its reasons before the quote, and offers "It's safe"

### Requirement: Moving through a stream with the arrow keys
In a stream's list, ↓ and ↑ SHALL move the keyboard cursor like J and K
(the first press lands on the first row) without opening the conversation,
and Enter SHALL open the row under the cursor. Moving past the last loaded row
SHALL load the next page, and the row the keyboard moves to SHALL be scrolled
into view. In an open conversation the arrow keys SHALL keep their browser
behaviour (scrolling it) while J and K step between conversations. Arrow keys
SHALL NOT be taken from a field, a radio group, a menu, a toolbar, a listbox,
a grid or an open dialog. The key list of the empty reader and the shortcut
sheet SHALL show both bindings ("J or ↓").

#### Scenario: Arrow down, then Enter
- **WHEN** the user presses ↓ three times, ↑ once, ↓ once and Enter in a stream
- **THEN** the cursor moves Q4, NDA, Ledger, NDA, Ledger and the Ledger conversation opens

#### Scenario: Arrows in an open conversation
- **WHEN** a conversation is open and the user presses ↓
- **THEN** the same conversation stays open and the browser scrolls it

#### Scenario: Arrows in a menu
- **WHEN** focus is in a menu, a field or a dialog and the user presses ↓
- **THEN** the list cursor does not move and the control handles the key

### Requirement: Kept inboxes are read as history
An inbox `conversations.list_inboxes` lists as `read_only` (its mailbox was
disconnected and its mail kept), or one the person only reads, SHALL open in
the reader with no action that writes: no Done, star, read or unread, move,
reply, reply all or forward, no assistant choices, and opening it SHALL NOT
mark it read. Until the inbox is known (its list loading or failed), the
reader SHALL offer no write action either. A kept inbox SHALL say so ("Kept as history", naming its
address). New messages SHALL never offer a read-only inbox to send from.

#### Scenario: A kept conversation
- **WHEN** the person opens a conversation of an inbox listed with `read_only: true`
- **THEN** "Kept as history" is shown, no write action is offered, E does nothing, and nothing is marked read
