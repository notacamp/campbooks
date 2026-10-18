# Tasks

## 1. The token

- [x] 1.1 `domain::binding::ContentToken` and `Shown` (draft or rule) under the #227 key, label `campbooks/conversations.content-token/v1`; base64url; unit tests for every bound field, another member, another key, no key, garbage
- [x] 1.2 `DraftRow::shown` and `rules::binding::content_token` over the same content hashes the signers bind

## 2. Views carry it

- [x] 2.1 `DraftView.content_token` on every draft response (read, list, create, save, send, undo)
- [x] 2.2 `RuleView.content_token` on every rule response

## 3. Signers require it

- [x] 3.1 `send_draft`: verify under the row lock after the version check; 409 `content_changed`, WARN ids only; one content hash for the check and the request MAC; 500 without a key
- [x] 3.2 `confirm_rule` (proposal and yes again) and `update_rule` with `adopt` (unless the whole rule is set)
- [x] 3.4 `update_draft` takes the token on every save, checked against the row before the write; its answer issues the next one (the chain from the first read to the send)
- [x] 3.5 The draft token binds the inbox id (the #552 content hash unchanged)
- [x] 3.3 `errors.content_changed` in the server catalogue, all four locales

## 4. Client

- [x] 4.1 The composer keeps the token of what is on screen, sends it, and on 409 shows the draft as it is now with a notice
- [x] 4.2 The message composer shows copies whenever there are any
- [x] 4.3 The reply dialog never reuses a draft it did not create: a fresh one with the recipients it shows, no copies, its quoted text, no attachments; the replaced one is discarded
- [x] 4.6 A refused autosave keeps the typed text, reads the draft again, says so once and saves nothing on its own; the forward dialog saves with the token and starts fresh when refused
- [x] 4.4 The rules list's Keep sends the rule's token and says when it changed
- [x] 4.5 i18n en/pt-PT/es-ES/fr

## 5. Tests

- [x] 5.1 A Bcc or body changed with SQL after the read: 409, nothing scheduled, signed or published
- [x] 5.2 No token, garbage, another member's token: 409; a stale token after a save: 409, then the save's token sends
- [x] 5.3 A proposal changed with SQL (actions, conditions, author) after the read: 409, still proposed, unsigned, no run; another member's token: 409
- [x] 5.4 A rule needing a yes again changed after the read: 409; adopt with a stale or missing token: 409, with the whole rule: no token needed
- [x] 5.5 Existing send, undo, schedule, confirm and adopt flows send the token they read
- [x] 5.6 Client: the 409 re-read in the composer, the rules list notice, copies that arrive later
- [x] 5.7 Another draft's or rule's token over HTTP: 409; hidden fields (an attachment's file, quoted text, reply headers) changed with SQL between open and autosave: the autosave answers 409 and the send signs nothing; a stale token after an autosave: 409, then the last save's works
- [x] 5.8 Client: an autosave refused keeps the text and saves with the fresh token; the reply dialog never sends what a reused draft held

## 6. Last

- [x] 6.1 OpenAPI regenerated, client types regenerated
- [x] 6.2 CHANGELOG updated
- [x] 6.3 Gates green in CI; UI recorded at 375px and desktop
