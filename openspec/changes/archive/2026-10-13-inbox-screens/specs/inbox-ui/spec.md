## ADDED Requirements

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
as suggestions and who placed the conversation, "Why?" SHALL list the stored
reasons in the user's language, and a stream suggestion SHALL offer
"Move to <stream>" (1), which the user can undo.

#### Scenario: Taking a suggestion
- **WHEN** the user opens a newsletter that the rules suggest for Newsletters and presses 1
- **THEN** it moves to Newsletters, the toast offers Undo, and "Why?" had listed the mailing-list header reason

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
