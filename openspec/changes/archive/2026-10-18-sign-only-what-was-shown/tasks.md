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
- [x] 3.3 `errors.content_changed` in the server catalogue, all four locales

## 4. Client

- [x] 4.1 The composer keeps the token of what is on screen, sends it, and on 409 shows the draft as it is now with a notice
- [x] 4.2 The message composer shows copies whenever there are any
- [x] 4.3 The reply dialog saves exactly the recipients it shows and sends with the save's token
- [x] 4.4 The rules list's Keep sends the rule's token and says when it changed
- [x] 4.5 i18n en/pt-PT/es-ES/fr

## 5. Tests

- [x] 5.1 A Bcc or body changed with SQL after the read: 409, nothing scheduled, signed or published
- [x] 5.2 No token, garbage, another member's token: 409; a stale token after a save: 409, then the save's token sends
- [x] 5.3 A proposal changed with SQL (actions, conditions, author) after the read: 409, still proposed, unsigned, no run; another member's token: 409
- [x] 5.4 A rule needing a yes again changed after the read: 409; adopt with a stale or missing token: 409, with the whole rule: no token needed
- [x] 5.5 Existing send, undo, schedule, confirm and adopt flows send the token they read
- [x] 5.6 Client: the 409 re-read in the composer, the rules list notice, copies that arrive later

## 6. Last

- [x] 6.1 OpenAPI regenerated, client types regenerated
- [x] 6.2 CHANGELOG updated
- [x] 6.3 Gates green in CI; UI recorded at 375px and desktop
