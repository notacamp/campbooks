## MODIFIED Requirements

### Requirement: The reply slot
The reader SHALL expose a slot for the send slice's composer, pinned to the
bottom of the reader after the thread and never between or after its
messages in the flow, with Reply (R), Reply all (Shift+R) and Forward (F) that
tell the slot which message to answer and how; without a composer those
actions SHALL be hidden. From 48rem Reply, Reply all and Forward SHALL sit
together in the reader's header right after Done, in that order and in one
style, so they stay on its first row when the header wraps, and the
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
- **THEN** the reader's header shows Done, then Reply (R), Reply all (Shift+R) and Forward (F) side by side on its first row, and no "Reply to <name>" button sits at the bottom of the reader

#### Scenario: A long thread on a desktop
- **WHEN** the member opens a twelve-message thread at 1440px and scrolls it
- **THEN** Reply stays in the header beside Reply all and Forward, nothing is pinned at the bottom, and no reply box sits between or after the messages
