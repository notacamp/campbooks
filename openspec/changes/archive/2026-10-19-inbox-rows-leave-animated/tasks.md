# Tasks

## 1. Client

- [x] 1.1 `ConversationRow`: `leaving` (label, icon, tone) wipes in over the row, then the row folds away and `onLeft` runs (`--duration-confirm`); not a button, hidden from assistive tech, instant under reduced motion
- [x] 1.2 `withLeaving` (conversations model): leaving rows back in place, each above the row it sat above
- [x] 1.3 Inbox: Done, Move to Inbox, Delete, Restore, Report spam, Not spam and a move to another stream mark the row as leaving when it leaves the list on screen; Undo during the fold drops it
- [x] 1.4 i18n: `leaving.*` in en, pt-PT, es-ES, fr

## 2. Tests

- [x] 2.1 `ConversationRow`: leaving row says what happened, is not a button, ends after 900 ms
- [x] 2.2 `withLeaving` unit tests (one after another, last row, row below gone, back on the list)
- [x] 2.3 Stories: `ConversationRow` Leaving and LeavingAsSpam; `Screens/Inbox` RowsLeave (desktop and phone), with a mailbox that keeps what is done to it

## 3. Docs

- [x] 3.1 inbox-ui spec: rows leaving the list
- [x] 3.2 Changelog entry
- [x] 3.3 Recording (375px + desktop)
