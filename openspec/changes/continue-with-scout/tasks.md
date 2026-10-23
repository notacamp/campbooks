# Tasks

## 1. Gateway tool calls (`campbooks-ai`)

- [x] 1.1 `ChatRequest.tools` (name, description, parameters schema) and `tool_choice`; `MessageRole::Tool` with `tool_call_id`; assistant messages carrying `tool_calls`
- [x] 1.2 `ChatResponse.tool_calls` (id, name, arguments text); `finish_kind` keeps `tool_calls`
- [x] 1.3 `ChatEvent::ToolCall` in streams: per-index argument chunks joined, yielded once complete, before `Done`
- [x] 1.4 Tests: whole and streamed tool calls, several in one turn, arguments not JSON handed over as they came, nothing logged

## 2. History read (`conversations`)

- [x] 2.1 `conversations.get_history` feature slice: events from messages, decisions (checked model answers and rules), corrections, rule applications, the need and its clock; oldest first, at most 200
- [x] 2.2 `x-campbooks`: query, `.assistant()`, records, access as `get_conversation`
- [x] 2.3 HTTP tests: a bill, a correction by the caller and by another member, a rule applied then reverted, 404 for an unreadable conversation, no message text

## 3. Chat storage (`assistant`)

- [x] 3.1 Migration `<slot>_assistant__chats`: `assistant.chats`, `assistant.chat_messages`, checks, the partial unique key, RLS enabled and forced, grants
- [x] 3.2 `assistant.chat` in the access catalogue, owned by its member
- [x] 3.3 Purge and `member_left` delete chats; daily schedule `assistant.dispatch_expire_chats` (maintenance scope, read-only policy, `architecture.toml` entry) enqueuing `assistant.expire_chats` per workspace (30 days after the last message)

## 4. Chat operations (`assistant`)

- [x] 4.1 `start_chat` (`about` a `RecordRef` of a registered type, else 400 `about_unsupported`), `get_chat`, `delete_chat`
- [x] 4.2 The handbook: the assistant's core section, `conversations` and `attention` sections in their crates (`HANDBOOK`), composed by the server in a fixed order, version derived from every part, each section at most 8 KiB (server test); the fixed instruction
- [x] 4.3 The about registry: `conversations.conversation` read as the member (`conversations.get`, `conversations.get_history`), bounded own words; registry ops checked against the catalogue
- [x] 4.3a The context: `now` (UTC), the member's `time_zone` (question body, checked against `pg_timezone_names`, else UTC) with the local date, time and weekday, and their language
- [x] 4.4 The tool registry: six tools, narrow schemas, compact projections, 8 KiB / 24 KiB bounds, unknown tools refused without invoking
- [x] 4.5 The question loop: up to 6 tool rounds then an answer without tools; steps; storing the question and the answer (`done`, `failed`, `cut_short`)
- [x] 4.6 `ask_chat` (whole) and `stream_chat` (SSE: `thinking`, `step`, `text`, `done`, `error`); a dropped stream stores `failed`
- [x] 4.7 Sanitised question text, `question_empty`, `question_too_long`, `chat_full`, `chat_unavailable`, `chat_failed`, interactive usage scope `assistant.chat`, 30 per hour
- [x] 4.8 Messages in en, pt-PT, es-ES and fr
- [x] 4.9 Logs and spans with sizes, rounds, tools and outcome only

## 5. Server tests

- [x] 5.1 Tool and about registries against the catalogue (`assistant: true`, `mutates: false`)
- [x] 5.2 HTTP tests with a fake provider: start and continue, another member's chat 404, an app acting for nobody 403, a streamed answer with steps, the desktop answer, a tool that does not exist, a hostile message stays data, access lost, limits and the cap, a full chat, a failed and a dropped answer, nothing personal in logs
- [x] 5.3 The access matrix seeds a chat; the BFF passes the stream through

## 6. Client

- [x] 6.1 `lib/`: the opener the app provides (`openScoutChat(conversationId, from)`), a no-op without a panel
- [x] 6.2 `assistant/api`: chat queries, `streamChat` (falls back to `ask_chat` where answers can't stream)
- [x] 6.3 `ScoutChat` panel: aside from `lg`, full-screen dialog below; about line; messages; questions to start from; thinking, steps, text; "What I looked at"; errors; Try again; Start over
- [x] 6.4 Mounted by `app/routes/workspace.tsx`; "Continue with Scout" on Home needs (attention) and "Ask Scout" in the reader (conversations), both on `.` (item scope) in the shortcut registry
- [x] 6.5 Translations in four locales; stories (screen stories with mocked streams) and component tests; `test:fit`
- [x] 6.6 A recording scenario at 375px and desktop

## 7. Close

- [x] 7.1 OpenAPI regenerated
- [ ] 7.2 Client types regenerated
- [x] 7.3 Changelog entry file (`changelog/unreleased/added/305-continue-with-scout.md`)
- [ ] 7.4 Gates green
- [ ] 7.5 UI verified at 375px and desktop (the lead records it)
- [ ] 7.6 `openspec archive continue-with-scout --yes` before merge
