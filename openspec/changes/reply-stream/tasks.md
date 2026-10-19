# Tasks

## 1. Specs

- [x] 1.1 OpenSpec change (proposal, design, spec deltas) and ADR 0019

## 2. Platform

- [ ] 2.1 `kernel::time_limit`: a `Deadline` per request, raised by a handler up to 100 s; the server uses it; tests
- [ ] 2.2 Idempotency: an event-stream answer releases its key and is returned untouched; test
- [ ] 2.3 AI: `ChatRequest.timeout`, owned `Call`, `REASONING_EFFORT` setting and `.env.example`
- [ ] 2.4 AI: `Gateway::chat_stream` with the SSE parser, `<think>` hygiene, metering and settling however it ends, one log line; tests

## 3. Conversations

- [ ] 3.1 `WriteReply.suggestions` validated; plain-text written reply; suggestions in the prompt
- [ ] 3.2 JSON routes: 90 s allowance, 80 s model timeout, `reply_timed_out` in four catalogues, timing logs
- [ ] 3.3 `stream.rs` driver (idle, total, caps, empty, length, report) with paused-time tests
- [ ] 3.4 `conversations.stream_reply` route, OpenAPI, HTTP tests, access matrix

## 4. Desktop

- [ ] 4.1 Tauri bridge timeout to 100 s

## 5. Client

- [ ] 5.1 `readEventStream`, `canStream`, `streamReply`, `mockApi` streams; tests
- [ ] 5.2 `ThinkingTrace` component, story and test
- [ ] 5.3 `ReplyDialog` streaming, abort, suggestions, rewrite restore, timeout copy; i18n in four locales; tests

## 6. Recording

- [ ] 6.1 `fake-complete` streams; `reply-stream` scenario

## 7. Close

- [ ] 7.1 OpenAPI regenerated
- [ ] 7.2 Client types regenerated
- [ ] 7.3 Changelog entry file (changelog/unreleased/)
- [ ] 7.4 Gates green
- [ ] 7.5 UI verified at 375px and desktop (recording)
