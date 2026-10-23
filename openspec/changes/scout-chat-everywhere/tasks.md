# Tasks

## 1. Server

- [x] 1.1 `conversations.get_rule_notice` (query, `.assistant()`, 404 rules as `list_rule_notices`, no notice pass) with HTTP tests
- [x] 1.2 About registry: `conversations.rule` (`conversations.get_rule`) and `conversations.rule_notice` (`conversations.get_rule_notice`) with projections; the registry test covers them
- [x] 1.3 `start_chat` without `about`: always a new chat; the context says it is about nothing
- [x] 1.4 `assistant.list_chats` (own chats, newest first, 30, first question cut at 120 characters); access matrix
- [x] 1.5 HTTP tests: rule and notice chats, 404 for another inbox's rule or notice, two chats about nothing, list only one's own

## 2. Client

- [x] 2.1 `lib/scout-chat`: `openScoutChat(about | null, { from, ask })`; phase 1 callers move to it
- [x] 2.2 Panel: about line and starter questions per kind; "Recent" menu from `list_chats`
- [x] 2.3 Entry points: Scout noticed, Teach Scout after save, rules list overflow, Reply and Nudge dialogs, Waiting on others rows
- [x] 2.4 ⌘K: "Ask Scout" and "Ask Scout: <text>"
- [x] 2.5 Four locales; stories; component tests; `test:fit`
- [x] 2.6 Recording scenario: ⌘K question, Recent, Ask Scout from Scout noticed, at 375px and desktop

## 3. Close

- [x] 3.1 OpenAPI and client types regenerated (CI artifacts)
- [x] 3.2 Changelog entry file
- [x] 3.3 Gates green
- [ ] 3.4 UI recorded at 375px and desktop
- [ ] 3.5 `openspec archive scout-chat-everywhere --yes` before merge
