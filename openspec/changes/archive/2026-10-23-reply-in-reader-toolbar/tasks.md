# Tasks

## 1. The reader

- [x] 1.1 Reply (R) joins Reply all and Forward in the reader's header from 48rem, the three right after Done
- [x] 1.2 Every action after Done is an icon button with its name and key in the tooltip (Move's trigger too); the phone-only Reply all / Forward copies merge into the group
- [x] 1.3 `lib/ui` `OverflowActions` (story + test): icons while they fit, else a "More actions" menu with names, keys and ↑/↓; the reader's Delete, Report spam, Pin, Read/Unread, Previous and Next use it; "More actions" in four locales
- [x] 1.4 The footer drops its "Reply to <name>" prompt and holds only the open composer; the now unused `replyName` goes
- [x] 1.5 A reader test: Reply, Reply all and Forward are adjacent header icons, and nothing named "Reply to …" remains
- [x] 1.6 The `threads` recording clicks the header's Reply on desktops too
