# Tasks

## 1. Contracts

- [x] 1.1 `outgoing-message.v1`, `conversations.send_requested.v1`, `connectors.send_completed.v1` schemas; contracts README

## 2. conversations

- [x] 2.1 Migration `conversations.drafts` (RLS, grants); scope `conversations:send`; `UNDO_SEND_SECONDS` setting
- [x] 2.2 Drafts: create (reply / reply all / forward prefill, forwarded attachments held), get, list, update (version), delete
- [x] 2.3 Send and cancel; `conversations.release_send` (outgoing document, basic formatting renderer, `send_requested`)
- [x] 2.4 `conversations.record_send` on `connectors.send_completed`; `draft_sent`, `send_failed`
- [x] 2.5 Tests: roles, privacy, prefill, versions, undo, resend, release, outcome, contracts

## 3. connectors

- [x] 3.1 Migration `connectors.sends`; `connectors.queue_send` consumer
- [x] 3.2 Gmail client: `messages.send` (multipart upload with `threadId`), lookup by `rfc822msgid`
- [x] 3.3 MIME from the outgoing document (`mail-builder`)
- [x] 3.4 `connectors.send_message`: attempts, lookup before retry, outcomes, index + batch of the sent message, `send_completed`
- [x] 3.5 Fake Google send + tests: thread, idempotency (redelivery, timeout after accept, 400, 5xx), sync after send

## 4. Client

- [x] 4.1 `lib/ui`: recipient chips field, composer (From, To/Cc/Bcc, subject, formatting bar, quoted history, attachments, send/undo) with stories and tests
- [x] 4.2 `modules/conversations`: draft API hooks, autosave, reply slot renderer for #100's reader, New message screen, `c` shortcut
- [x] 4.3 i18n en / pt-PT / es-ES / fr

## 5. Finish

- [x] 5.1 OpenAPI, events catalogue and client types regenerated; `.sqlx` regenerated
- [x] 5.2 CHANGELOG updated
- [x] 5.3 Gates green
- [x] 5.4 UI verified and recorded at 375px and desktop (fake Google); vhs clip of the idempotency tests
