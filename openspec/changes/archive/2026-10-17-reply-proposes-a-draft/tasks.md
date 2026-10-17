# Tasks

## 1. Server: the reply slice in `conversations`

- [x] 1.1 `features/reply`: request and response types, the fixed prompt
      (mail only as JSON data, strict schemas), the thread loader (newest
      8 messages up to the answered one, text or HTML reduced to text,
      quoted history dropped, bounded).
- [x] 1.2 `conversations.suggest_replies` and `conversations.write_reply`:
      read on the conversation then send on its inbox, 409
      `reply_unavailable` without a `complete` model that may read mail,
      per-member hourly limit, interactive usage scope, 503 `reply_failed`.
- [x] 1.3 Catalogue reasons in en, pt-PT, es-ES and fr.
- [x] 1.4 Tests with a wiremock model: intents, draft, rewrite, prompt
      shape and injection, unavailable, external refused, provider error,
      reader 403, stranger 404, message of another conversation 404.
- [x] 1.5 Access matrix: bodies for both operations, acting in the inbox.

## 2. Client

- [x] 2.1 `lib/ui` `ReplyDraft` (draft editor with rewrite buttons, ask
      line, quoted context): story with every state, test.
- [x] 2.2 `conversations` API hooks: suggestions (cached per message for
      the session), write, send through `create_draft` + `send_draft`,
      undo through `cancel_send`.
- [x] 2.3 `ReplyDialog`: intents, writing, draft; keys 1–3, ⌘↵, Esc;
      Shorter, Change…, Open in composer, Write my own; unavailable and
      failure lines.
- [x] 2.4 Home: Reply opens the dialog; Send folds the row with its
      confirmation; an undone send brings row and dialog back; no Reply on
      a read-only inbox.
- [x] 2.5 Reader: Reply opens the dialog; Write my own and Open in
      composer open the composer on the draft.
- [x] 2.6 i18n in en, pt-PT, es-ES and fr; tests for the dialog and both
      entry points.

## 3. Finish

- [x] 3.1 OpenAPI regenerated (`openapi/openapi.json`).
- [x] 3.2 Client types regenerated (`schema.gen.ts`).
- [x] 3.3 Recording scenario `reply-dialog` and `scripts/fake-complete.ts`.
- [x] 3.4 CHANGELOG updated.
- [x] 3.5 Gates green in CI.
- [ ] 3.6 UI verified at 375px and desktop (the lead, by hand, before merge).
