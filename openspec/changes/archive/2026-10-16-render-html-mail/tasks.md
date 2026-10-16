# Tasks

## 1. Diagnose
- [x] 1.1 Fixture emails (newsletter, receipt, Outlook, Gmail, fixed 600px, fluid marketing, hostile) under `crates/modules/conversations/fixtures/email-html`, benign ones adapted from MIT templates, fictional content only
- [x] 1.2 Render each raw and through the old sanitiser and frame at 375px and desktop (Chromium, WebKit); measure the frame against the content; record what breaks

## 2. Server
- [x] 2.1 `sanitize::styles`: inline style sheets with css-inline (no fetching, no file reading, 256 KiB cap), keep the body's look for a wrapper, keep at-rules back
- [x] 2.2 `sanitize::css`: declaration filter on cssparser with a function allow-list; `@media` rules filter
- [x] 2.3 Sanitiser: `class` kept, `<title>` dropped with its content, wrapper for the body, pixels by style
- [x] 2.4 `get_message_body` returns `css`
- [x] 2.5 Tests over every fixture (inert output, hostile cases), the CSS filter, the inliner, and the endpoint

## 3. Client
- [x] 3.1 `SafeHtml`: `allow-same-origin` (never `allow-scripts`), measured height with a `ResizeObserver`, fit by `zoom`, `@media` rules as a constructed sheet; estimate kept as the first height
- [x] 3.2 Reader passes `css`
- [x] 3.3 Unit tests (exact sandbox tokens, no CSS in the markup, fit) and stories with play tests (long message, wide layout, responsive rules)
- [x] 3.4 Recording scenario `html-mail` and the fake Google's `--html-mail` mailbox

## 4. Finish
- [x] 4.1 OpenAPI regenerated
- [x] 4.2 Client types regenerated
- [x] 4.3 CHANGELOG updated
- [x] 4.4 Gates green
- [x] 4.5 UI verified at 375px and desktop, before and after, with a recording
- [x] 4.6 Security review of the diff, findings fixed
