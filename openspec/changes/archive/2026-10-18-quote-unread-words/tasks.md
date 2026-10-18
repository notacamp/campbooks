## 1. Server

- [x] 1.1 `words=true` on `conversations.get_message_body`: the words alone, bounded, same access
- [x] 1.2 Debug-only `CAMPBOOKS_DEV_TEXT_DEADLINE_MS` for `conversations.get`'s deadline
- [x] 1.3 Test: HTML-only words, a text part, another member gets 404

## 2. Client

- [x] 2.1 Words query per message, enabled for `text_unread`; `withWords` before quoting
- [x] 2.2 Composer reply and forward, Home's Forward picker, the reply dialog, the folded line
- [x] 2.3 Neutral reading state; nothing started or sent when the words cannot be read
- [x] 2.4 Tests for reply, forward (composer and picker), the reply dialog and the folded line
- [x] 2.5 i18n en, pt-PT, es-ES, fr

## 3. Recording

- [x] 3.1 Scenario `forward-text-unread` on a debug server with the deadline at 0
