## Why

The conversations module stores, threads and sorts mail, but the app has no
inbox: its client module is a placeholder. A working inbox is the owner's
priority before the production cutover (campbooks-cloud#94). Reading untrusted
email HTML also needs a safe path end to end, and the decide pass's answers
need to reach the member as what they are in this version: suggestions with
reasons, never silent moves.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/100.
Modules touched: `conversations` (server crate and client module), the
server's web delivery (its Content-Security-Policy), the app shell
(navigation), and `lib/ui` (components). No new module, no cross-module
dependency.

## What Changes

- **Streams in the shell.** Inbox joins the sidebar and the phone tab bar,
  with its four streams (Primary, Updates, Receipts, Newsletters) under it,
  each with its unread count and a shortcut (G then 1–4). There is no
  "People" umbrella: the inbox is its streams.
- **Conversation list** per stream (`/w/$workspaceId/inbox/$stream`):
  newest first, a page at a time, unread / starred / message count, All or
  Unread, and the decide pass's stream answer on the row as a suggestion
  ("Suggests Newsletters"). J/K move, Enter opens, E Done, S star, U read.
- **Reader** (`…/inbox/$stream/$conversationId`): actions with their keys
  (Done E, star S, read/unread U, move M with 1–4, previous/next K/J, back
  Esc), the assistant's note (what the decide pass thinks, as suggestions;
  "Why?" lists the stored reasons; a stream suggestion offers Move (1)),
  the thread newest first with older messages folded, attachments listed,
  and a **reply slot** the send slice fills (Reply R, Reply all A, Forward F).
  Opening an unread conversation reads it. Every action pushes an Undo (Z).
- **Phones**: below 64rem list and reader are separate screens; below 48rem
  the streams are chips and the reader's main actions float above the tab
  bar.
- **Email HTML safety, two layers.** `conversations.get_message_body`
  sanitises the HTML with ammonia (no scripts, style sheets, forms, frames or
  handlers; safe CSS declarations only; links open in a new tab without an
  opener), removes tracking pixels, and holds remote images back (counted)
  unless `show_images=true`. The client shows it only in a sandboxed frame
  (no scripts, opaque origin, its own CSP, no referrer), with "Show images"
  per message (Shift+I for all).
- **New read operation** `conversations.list_streams` (`GET
  /api/v1/w/{workspace_id}/streams`): every stream with its conversation and
  unread counts.
- **Suggestions in the API**: `ConversationSummary.suggestion` (the decide
  pass's stream answer when it did not act and names another stream) and
  `ConversationDetail.decisions` (every answer about the newest inbound
  message, with its reasons and whether it acted).
- **Web CSP** gains `style-src-attr 'unsafe-inline'` and `img-src https:`, the
  most the sandboxed frame needs (it inherits the page's policy); the frame's
  own policy stays stricter.
- **Fake Google**: a demo Gmail mailbox for recordings and development.

## Non-goals

- Sending, drafts, the composer and attachments download (the send slice,
  campbooks-cloud#101, fills the reply slot).
- Search, labels in the UI, snooze/Later, bulk selection, custom streams and
  stream rules ("Edit the rule").
- Applying decide answers automatically: the shipped policy still lets none
  act; the UI only shows them as suggestions.
- Remembering "Show images" per sender, an image proxy, and inline (`cid:`)
  images.
- The native (Tauri) change-feed source.
