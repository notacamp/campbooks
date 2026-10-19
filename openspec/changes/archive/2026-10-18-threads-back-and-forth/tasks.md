# Tasks

## 1. Server

- [x] 1.1 `domain::sanitize::quotes`: mark a reply's trailing quoted history with `<div data-cb-quoted="1">`, from structure only
- [x] 1.2 Allow `type="cite"` on `blockquote` and Outlook's two ids on `div`, those values only
- [x] 1.3 Tests: each provider shape, trailing only, inline replies, nested quotes, a spoofed mark
- [x] 1.4 `latest_from_you` on `ConversationSummary` from the list and summary queries; offline query cache updated
- [x] 1.5 List test: who wrote last

## 2. Design system

- [x] 2.1 `MessageCard` / `FoldedMessage` `side` and `addressee`; `MoreMessages`; `QuotedText`; stories and tests
- [x] 2.2 `ReaderFooter` in `Shell`; stories and tests
- [x] 2.3 `SafeHtml` folds the marked block in a `details`; story and tests
- [x] 2.4 Component tokens `--message-yours-indent`, `--message-yours-bg`, `--reader-footer-max`

## 3. Inbox

- [x] 3.1 `threadLayout` and `splitQuoted` in `model.ts`, with tests
- [x] 3.2 Reader: lanes, "N more messages", quoted text, the pinned reply area
- [x] 3.3 List rows: "You:" before the snippet
- [x] 3.4 Translations in en, pt-PT, es-ES and fr

## 4. Wrap-up

- [x] 4.1 OpenAPI and client types updated
- [x] 4.2 CHANGELOG updated
- [x] 4.3 Gates green in CI
- [x] 4.4 UI verified at 375px and desktop (recording on the demo stack)
