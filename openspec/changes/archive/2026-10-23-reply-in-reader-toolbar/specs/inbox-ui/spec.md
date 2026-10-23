## MODIFIED Requirements

### Requirement: The reply slot
The reader SHALL expose a slot for the send slice's composer, pinned to the
bottom of the reader after the thread and never between or after its
messages in the flow, with Reply (R), Reply all (Shift+R) and Forward (F) that
tell the slot which message to answer and how; without a composer those
actions SHALL be hidden. From 48rem Reply, Reply all and Forward SHALL sit
together in the reader's header right after Done, in that order, then Move;
every action after Done SHALL be an icon button whose tooltip names it and
its key. Delete, Report spam, Pin, Read or Unread, and Previous and Next at
the end, SHALL show as icons while the header has room for them, and
otherwise fold into one "More actions" menu (each row with its name and
key, ↑/↓ between rows, Esc closes), so the header is one row at every
width; their keys work either way. The
pinned area SHALL hold only the open composer: it stays in view while the
thread scrolls under it, growing upward to 60% of the window's height and
then scrolling inside itself; while the composer is closed nothing SHALL be
pinned there. Below 48rem the floating bar above the tab bar SHALL offer
Reply and Done, and an open composer SHALL take its place. Reply SHALL first
open the reply dialog (Scout's likely replies and draft); its "Write my own"
and "Open in composer" SHALL open the slot's composer on the newest message,
the latter on the draft Scout wrote. Reply all and Forward SHALL open the
composer directly.

#### Scenario: No composer yet
- **WHEN** the inbox runs without a reply-slot renderer
- **THEN** the reader shows no Reply, Reply all or Forward

#### Scenario: Write my own
- **WHEN** the user presses R in the reader, then chooses "Write my own"
- **THEN** the dialog closes and the composer opens in the pinned area at the bottom of the reader

#### Scenario: Reply beside Reply all and Forward
- **WHEN** the member opens a conversation at 1280px
- **THEN** the reader's header is one row: Done, then Reply (R), Reply all (Shift+R) and Forward (F) side by side as icons whose tooltips name them and their keys, and no "Reply to <name>" button sits at the bottom of the reader

#### Scenario: A narrow reader folds the rest into More
- **WHEN** the member opens a conversation at 1100px wide, where the reader is too narrow for every icon
- **THEN** the header is still one row: Done, Reply, Reply all, Forward and Move, then "More actions" holding Delete, Report spam, Pin, Mark unread, Previous and Next with their keys, and # still deletes

#### Scenario: A long thread on a desktop
- **WHEN** the member opens a twelve-message thread at 1440px and scrolls it
- **THEN** Reply stays in the header beside Reply all and Forward, nothing is pinned at the bottom, and no reply box sits between or after the messages
