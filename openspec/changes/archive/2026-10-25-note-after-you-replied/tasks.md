# Tasks

## 1. The reader's note

- [x] 1.1 `repliedSince(messages)` in the conversations model: when the member last wrote after the newest inbound message, or null
- [x] 1.2 `noteSentence` and `suggestsReply` take it: after a reply the note says "You replied on <date>…", keeps only a suspicious warning, and offers no Reply
- [x] 1.3 The reader passes the thread's messages; "scout.replied" in en, pt-PT, es-ES and fr
- [x] 1.4 Model tests (a reply before the newest inbound one does not count; the newest reply's date; a warning stands) and a reader test (no "needs a reply", no Reply button)
- [x] 1.5 Recording at 375px and desktop
