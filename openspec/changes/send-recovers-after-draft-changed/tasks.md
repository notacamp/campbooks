# Tasks

## 1. Client

- [x] 1.1 `changedElsewhere(thrown)` in `api/drafts.ts`: `draft_changed` or `content_changed`, shared by the editor and both dialogs
- [x] 1.2 `useDraftEditor.send`: on either code, read the draft again (`showCurrent`, or only version and token when the member typed meanwhile), show the `content_changed` notice, never resend
- [x] 1.3 `ReplyDialog.send`: on either code (save or Send), say so, drop the saved draft and replace it with a fresh one on the next Send
- [x] 1.4 `ForwardDialog`: start a fresh forward on either code
- [x] 1.5 Remove the unreachable `compose.errors.draft_changed` notice (en, pt-PT, es-ES, fr)

## 2. Tests

- [x] 2.1 Composer: Send → 409 `draft_changed` → re-read → the next Send carries the fresh version and token and succeeds
- [x] 2.2 Reply dialog: Send → 409 `draft_changed` → the next Send goes in a fresh draft; its save → 409 `draft_changed` → the next Send goes in a fresh draft
- [x] 2.3 Forward dialog: the untouched forward's save → 409 `draft_changed` → a fresh forward opens

## 3. Recording

- [x] 3.1 `draft-changed` scenario (375px and desktop), demo data and page-level mocks only
