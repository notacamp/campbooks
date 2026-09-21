# Scout overlay, chat & tools — `/api/app`

**Status:** 🟡 partial — `/api/v1` has async chat threads/messages only · **Priority:** P2 · **Depends on:** [`00-auth.md`](00-auth.md)

## What the SPA must render

- **Docked Scout bar** (every surface): quick message input, unread-reply dot (lights when `AgentMessage.viewed_at IS NULL` for AI messages; clears on Scout visit), last reply preview.
- **Cmd+K overlay** (lazy-loaded, currently `ScoutOverlayController#show`): idle state — suggestions from `Scout::Briefing`, last 6 recent threads, command catalog + live search (client-side today via `scout_overlay_controller.js`); conversation state — last 20 messages of a chosen thread, streaming reply in place.
- **Full Scout surface** (`/scout`, `AgentChatController`): thread list (sidebar, last 30 with messages), active conversation panel (last 50 messages), typing/status indicator, AI reply, suggested-action chips (confirm-level tool cards the user taps), thread CRUD.
- **Tool confirmation cards**: confirm-level tools surfaced as tappable cards in the reply (`bulk_archive`, `bulk_tag`, `reclassify`, etc.) — dispatched via `AgentToolsController`; unsafe tools are blocked server-side.
- **Thread management**: create, rename (`update`), delete (`destroy`).

## Web routes being replaced

| Web route | Controller#action | Notes |
|---|---|---|
| `GET /scout` | `agent_chat#show` | Full Scout surface; loads last 30 threads + last 50 messages of default thread |
| `POST /scout` | `agent_chat#create` | Post a message to the default global thread; Turbo Stream reply |
| `GET /scout/threads/:id` | `agent_threads#show` | Thread detail with messages |
| `POST /scout/threads` | `agent_threads#create` | Create a new thread |
| `PATCH /scout/threads/:id` | `agent_threads#update` | Rename thread |
| `DELETE /scout/threads/:id` | `agent_threads#destroy` | Delete thread |
| `POST /scout/threads/:thread_id/messages` | `agent_messages#create` | Post message; enqueues `AgentChatReplyJob` |
| `POST /scout/tool` | `agent_tools#create` | Execute a confirm-level tool via `Scout::ToolRegistry.run` |
| `GET /scout/overlay` | `scout_overlay#show` | Lazy-loaded overlay content (idle or conversation mode) |

## `/api/v1` coverage today

🟡 **Partial — chat only.** `GET/POST /api/v1/scout/threads` (index/create, scoped `scout:read`/`scout:write`) and `GET/POST /api/v1/scout/threads/:thread_id/messages` (index with `?after_message_id` poll, create → 202 async). Serializers `Api::V1::AgentThreadSerializer` and `Api::V1::AgentMessageSerializer` exist and are reusable.

**Not covered:** overlay idle payload (suggestions, recent threads), tool invocation/confirmation, unread count, thread rename/delete, typing-status events — all net-new.

## `/api/app` endpoints to build

| Method + path | Purpose | Reuses |
|---|---|---|
| `GET  /api/app/scout/overlay` | Idle state: AI-available flag, suggestions, last 6 threads | `Scout::Briefing.for(user)`, `AgentThread.scout_visible.with_messages.recent.limit(6)` |
| `GET  /api/app/scout/threads` | Paginated thread list (scout_visible, with_messages, recent) | `AgentThread` scopes; `Api::V1::AgentThreadSerializer` |
| `POST /api/app/scout/threads` | Create a new thread | `AgentThread#create`; same logic as v1 |
| `GET  /api/app/scout/threads/:id` | Thread header + last 50 messages | `AgentThread`, `AgentMessage`; reuse serializers |
| `PATCH /api/app/scout/threads/:id` | Rename thread (title) | `AgentThreadsController#update` logic |
| `DELETE /api/app/scout/threads/:id` | Delete thread | `AgentThreadsController#destroy` logic |
| `POST /api/app/scout/threads/:thread_id/messages` | Post user message → 202; enqueue AI reply | `AgentChatReplyJob`; `Api::V1::AgentMessageSerializer` |
| `GET  /api/app/scout/threads/:thread_id/messages` | Poll for new messages (`?after_message_id=N`) | Same poll logic as v1 `ScoutMessagesController#index` |
| `POST /api/app/scout/tool` | Execute a confirm-level tool; return result + toast text | `Scout::ToolRegistry.run(name, args)`; `AgentToolsController` logic |
| `GET  /api/app/scout/unread` | Count of unviewed AI replies across all visible threads | `AgentMessage.where(viewed_at: nil, author_type: :ai)` scoped to user |
| `POST /api/app/scout/mark_read` | Mark all AI replies as viewed (clear nav dot) | `AgentMessage#update_all(viewed_at: Time.current)` |

**Tool dispatch detail:** `AgentToolsController` calls `Scout::ToolRegistry.find(tool)` then `Scout::ToolRegistry.run(tool, args)` — confirm-level only; unsafe tools remain blocked. `Tools::Executor.call(tool:, email_message:, args:)` is the underlying shim into `EmailActions.run`. Tool categories in `app/services/tools/`:
- **Email actions**: `archive`, `snooze`, `unsnooze`, `trash`, `unarchive`, `add_tag`, `remove_tag`, `forward_email`, `draft_reply`, `draft_follow_up`, `reclassify`
- **Bulk**: `bulk_archive`, `bulk_delete`, `bulk_forward`, `bulk_mark_read`, `bulk_move_to_folder`, `bulk_process_ai`, `bulk_scout_chat`, `bulk_snooze`, `bulk_tag`, `bulk_unarchive`, `bulk_unsnooze`
- **Query**: `query_emails`, `query_contacts`, `query_documents`, `query_asks`
- **Calendar/Tasks**: `create_calendar_event`, `create_task_from_email`, `link_task_to_email`
- **Documents/Integrations**: `generate_report`, `upload_email_attachments_to_drive`
- **System**: `system_stats`

## Read models / serializers needed

- **`Api::V1::AgentThreadSerializer`** — reuse; extend to `Api::App::` if `suggested_actions` or `reply_status` fields differ.
- **`Api::V1::AgentMessageSerializer`** — reuse; verify it exposes `author_type`, `reply_status`, `ai_suggested_actions`, `tool_invocations`, `viewed_at`.
- **`Api::App::ScoutOverlaySerializer`** (new) — `{ ai_available: bool, suggestions: [], recent_threads: [] }`.
- **`Api::App::ToolResultSerializer`** (new) — `{ tool: string, success: bool, result: {}, toast_message: string }`.

## Real-time

`AgentChatReplyJob` broadcasts on the Turbo Stream channel **`agent_chat_<user_id>`**:
- `broadcast_append_to` — new AI message appended
- `broadcast_replace_to` — typing/status indicator replaced (label: "Thinking…", "Reading emails…", etc.)
- `broadcast_remove_to` — typing indicator removed on reply or error

**For the SPA:** replace with a JSON ActionCable channel (e.g. `ScoutChannel`, subscribed by user bearer token). The job's `broadcast_reply` / `broadcast_typing_status` / `broadcast_error` methods need a dual-broadcast path — HTML Turbo Stream for the legacy Hotwire coexistence period, JSON events for the SPA:
- `{ type: "message", data: <AgentMessageSerializer> }` — new AI reply
- `{ type: "typing", label: <string> }` — tool/thinking status
- `{ type: "error", message: <string> }` — reply failed

Design decision lives in [`realtime.md`](realtime.md). Until then the SPA can fall back to the `?after_message_id` poll (already in v1).

## Open questions

- **Dual broadcast vs clean cut-over:** add JSON broadcasts to `AgentChatReplyJob` alongside the Turbo Stream ones (dual) or add a feature-flag path that skips the HTML broadcast entirely once Hotwire is gone?
- **Command catalog ownership:** today the catalog is rendered client-side by `scout_overlay_controller.js`. Does `/api/app/scout/overlay` serve the catalog schema (tool name + label + description), or does the SPA bundle it statically?
- **Default vs explicit thread:** `AgentChatController` uses `AgentThread.default_for(current_user)` (latest global thread with messages, or new one). Does the SPA always create a thread explicitly via `POST /api/app/scout/threads`, or does the API expose the same `default_for` semantics through a dedicated endpoint?
- **Overlay conversation mode:** `ScoutOverlayController#show` accepts `?thread_id=` or `?current=` to render conversation mode in the overlay. The SPA probably handles this entirely client-side — confirm no server round-trip needed beyond `GET /api/app/scout/threads/:id`.
