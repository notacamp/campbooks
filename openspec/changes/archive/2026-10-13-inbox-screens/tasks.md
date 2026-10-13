## 1. Server (conversations)

- [x] 1.1 `domain::sanitize` (ammonia): tags, attributes, schemes, links, style declarations; image policy with tracking pixels and remote images; unit tests for scripts, handlers, CSS, links, images and pixels
- [x] 1.2 `get_message_body`: sanitised HTML, `show_images`, `remote_images`, `images_shown`
- [x] 1.3 `list_streams` operation with counts, and its test
- [x] 1.4 `domain::suggestions`: `ConversationSummary.suggestion`, `ConversationDetail.decisions`, and a test under the shipped policy
- [x] 1.5 Web CSP: `style-src-attr 'unsafe-inline'`, `img-src https:`

## 2. Client

- [x] 2.1 `lib/ui`: `SafeHtml` (sandboxed frame, own CSP), `FoldedMessage`, richer `MessageCard` and `ConversationRow`, panes with `narrow`, `ActionBar` phone options, icons; stories and tests
- [x] 2.2 `conversations` module: queries with optimistic actions, routes, InboxScreen, ConversationList, ConversationReader, ThreadMessage, MovePicker, StreamNav/StreamChips, reply slot
- [x] 2.3 Shell: Inbox and its streams in the sidebar and tab bar; module shortcuts
- [x] 2.4 i18n en, pt-PT, es-ES, fr
- [x] 2.5 Screen stories for every state; module tests

## 3. Dev and recording

- [x] 3.1 Fake Google: demo Gmail mailbox, refresh grants, `/fake/deliver`
- [x] 3.2 `pnpm record inbox` scenario on the real stack

## 4. Wrap-up

- [x] 4.1 OpenAPI regenerated, client types regenerated
- [x] 4.2 CHANGELOG updated
- [x] 4.3 Gates green
- [x] 4.4 UI verified at 375px and desktop
