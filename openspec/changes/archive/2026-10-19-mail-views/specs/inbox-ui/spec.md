## MODIFIED Requirements

### Requirement: Streams, not people
The inbox SHALL be its streams: Primary, Updates, Receipts and Newsletters,
listed under Inbox in the sidebar (chips on phones) with their unread counts,
opened with G then 1 to 4. A stream's page SHALL list its pinned
conversations first, then the rest newest first, a page at a time, with
sender, time, subject, snippet, unread, pinned and message count, and All or
Unread. Four views SHALL sit under the streams, after a quiet divider: Sent,
Archive, Spam and Trash, each with its icon and no count, opened with G then
5 to 8. On phones they SHALL follow the stream chips, after a divider, in the
same row, which scrolls sideways on its own; the page never does. Views are
not streams: nothing is sorted into them, and a conversation can be in a
stream and a view at once. Any other place in the address SHALL open Primary.

#### Scenario: Open a stream from the keyboard
- **WHEN** the user presses G then 3
- **THEN** the Receipts stream opens

#### Scenario: A pinned conversation
- **WHEN** the user pins an older conversation in Primary
- **THEN** it moves to the top of Primary, marked pinned, and stays there as newer mail arrives

#### Scenario: Open a view from the keyboard
- **WHEN** the user presses G then 8
- **THEN** Trash opens, marked as the current place in the sidebar

#### Scenario: Views on a phone
- **WHEN** the inbox is open at 375px
- **THEN** the chips show the four streams, a divider, then Sent, Archive, Spam and Trash, and only the chip row scrolls sideways

#### Scenario: An empty view
- **WHEN** Spam has nothing in it
- **THEN** it says "Nothing in Spam." in one line

## ADDED Requirements

### Requirement: Delete and Report spam with undo
Delete (#) SHALL move the open conversation, or the one under the list's
cursor, to the trash, and Report spam (!) SHALL report it as spam, each shown
at once, with a toast ("Deleted · <subject>", "Reported as spam · <subject>")
and an entry that Undo (Z, or the toast) takes back, putting the conversation
back exactly where it was: Report spam on an archived conversation is undone
by Not spam and archiving it again, as one entry with one toast, while Not
spam on its own still brings received mail to the inbox. When it leaves the list
on screen, the next conversation opens (the list, on phones), as after Done.
The shortcuts SHALL be keyed by their operations (`conversations.trash`,
`conversations.report_spam`) and shown wherever their buttons are. Both SHALL
be icon buttons beside Pin and Mark read: one button style per toolbar.

#### Scenario: Delete and Undo
- **WHEN** the user presses # on an open conversation, then Z
- **THEN** it leaves the stream for Trash, the toast says "Deleted · <subject>", and Z restores it

#### Scenario: Report spam
- **WHEN** the user presses ! on a conversation in Primary
- **THEN** it leaves Primary for Spam, with Undo

#### Scenario: Undo Report spam from the archive
- **WHEN** the user presses ! on an archived conversation, then Z
- **THEN** it is out of Spam and back in Archive, not the inbox, in one Undo

### Requirement: The way back from each view
The reader's main action SHALL follow where the conversation is. In the inbox
it is Done (E). Archived (in Archive, or in Sent when archived) it SHALL be
Move to Inbox, on the same E. In Spam it SHALL be Not spam, on !, and in Trash
Restore, on #: the key that sent it there brings it back. Each SHALL have its
toast and Undo. In Spam and Trash the way back SHALL be the banner's button,
and E does nothing there. Spam SHALL offer no Delete, and # does nothing there:
moving it to the trash would clear its spam mark, and Not spam is its only way
out.

#### Scenario: No Delete in Spam
- **WHEN** the user opens a conversation in Spam and presses #
- **THEN** nothing happens; there is no Delete button, and ! (Not spam) is the way out

#### Scenario: Move to Inbox
- **WHEN** the user opens a conversation in Archive and presses E
- **THEN** it moves back to the inbox, the toast says "Moved to Inbox · <subject>", with Undo

#### Scenario: Restore
- **WHEN** the user opens a conversation in Trash and presses #
- **THEN** it goes back where it was, in the inbox or the archive, with Undo

#### Scenario: Not spam
- **WHEN** the user opens a conversation in Spam and presses !
- **THEN** its received mail is back in the inbox, with Undo

### Requirement: Sent rows
In Sent each row SHALL lead with whom the inbox's latest message there went
to ("To Marta Ferreira", "To Marta Ferreira and 2 others"), with that
person's avatar, and show the time of that message; it SHALL carry a
"Replied" mark when someone wrote back since. The list SHALL be ordered by
the inbox's latest message.

#### Scenario: They replied
- **WHEN** Maya answers a conversation the user wrote in last week
- **THEN** its Sent row stays where the user's message puts it and says "Replied"

### Requirement: Reading spam and the trash
A conversation in Spam SHALL say so in a plain banner ("This conversation is
in Spam.") with Not spam; one in Trash the same ("This conversation is in
Trash.") with Restore. In either, the reader SHALL offer no reply box, no
Reply, Reply all or Forward, and none of Scout's suggestions, needs or
drafts; restoring brings them back. In Spam remote images SHALL never load:
Show images, its ⇧I and "Always show images from …" are not offered, even
for a sender a rule trusts (the server says `images_held: spam`), and links
are inert as in mail that may be phishing.

#### Scenario: A trusted sender in Spam
- **WHEN** the user opens a reported newsletter from a sender a smart rule trusts
- **THEN** the banner says it is in Spam, its images stay held back with "Images never load in Spam.", and no Show images is offered

#### Scenario: No reply in the trash
- **WHEN** the user opens a conversation in Trash
- **THEN** there is no reply box and no Reply, and Restore brings both back
