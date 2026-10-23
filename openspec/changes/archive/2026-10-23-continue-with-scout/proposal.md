## Why

campbooks-cloud#305. The owner: "when i have an interaction with the ai
agent it should be possible to continue it over a chat to expand more on the
subject. for chats to be productive the agent needs to know how the system
works and what happened in it so it can answer user questions and take
actions".

Today every Scout interaction is one shot: a need's summary and ranked
actions, a reply draft, a nudge, Scout noticed. When the member wants to ask
"why is this under Pay?" or "has she written about this before?", there is
nowhere to ask. The owner approved the design and prototype on 2026-10-23
(https://claude.ai/artifact/TNNXk4sBjE9unmC5W24UbG): one Scout panel,
opened from the thing the member is looking at, that knows how Campbooks
works and can read what happened. It ships in three phases; this change is
phase 1, **Ask about this**: Scout explains and looks things up, and changes
nothing.

## What changes

- The `campbooks-ai` gateway learns tool calls: tool definitions on a chat
  request, tool calls in a response and in a stream, and tool results as
  messages.
- The `assistant` module gets chats: a member-private chat per conversation
  (or none), its messages, and the operations to start one, read it, ask in
  it (streamed on the web, whole on the desktop) and delete it. Chats are
  kept 30 days after their last message and deleted when the member leaves
  or the workspace is deleted.
- Each answer is written by the `complete` role from three layers: a fixed
  handbook of how Campbooks works, the conversation the chat is about
  (assembled by the server as the member), and read-only tools, each backed
  by a catalogue operation the assistant may call, run as the member
  through `kernel::operations::invoke`.
- Every answer carries the steps Scout took ("What I looked at"), built by
  the server from the tools that ran, never from the model's words.
- The `conversations` module gets one read, `conversations.get_history`:
  what happened to a conversation and who did it.
- The client gets the Scout panel (a side panel on desktop, full screen
  below `lg`) and two ways in: "Continue with Scout" on a Home need and
  "Ask Scout" in the reader, both on `.`.

## Non-goals

- Scout taking actions from a chat (phase 2: confirmation cards, rules
  through the smart-rule confirmation, drafts into the Reply dialog).
- Other entry points (Scout noticed, Nudge, Worth a look, teaching
  results, reply drafts), the recent chats list and ⌘K "Ask Scout" (phase 3).
- Feeding chat steps to the assistant activity page (campbooks-cloud#89).
- Search over documents, embeddings, or chat across workspaces.

## Capabilities

### New Capabilities

- `assistant-chat`: chats, their operations, how an answer is grounded and
  bounded, the gateway's tool calls, privacy and retention.
- `assistant-chat-ui`: the Scout panel.

### Modified Capabilities

- `conversations`: `conversations.get_history`.
- `home-ui`: "Continue with Scout" on a need.
- `inbox-ui`: "Ask Scout" in the reader.

## Modules

`assistant` (owner of chats), `conversations` (the history read),
`campbooks-ai` (tool calls), client modules `assistant`, `attention` and
`conversations`, composed by `app/`. No new cross-module crate dependency:
the assistant reads conversations only through operations invoked by id
(ADR 0013), as `assistant.compile_instruction` already does.
