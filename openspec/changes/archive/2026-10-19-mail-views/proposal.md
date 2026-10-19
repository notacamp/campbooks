## Why

The app shows only the inbox. Sent mail, archived mail, spam and the trash
are already in the database (Gmail syncs everything in the backfill window,
spam and trash included; Zoho walks Inbox, Sent, Spam and Trash), and the
trash, spam, not spam and restore operations exist, but no screen lists that
mail and no button calls them. The owner: the app is missing basic folders
(campbooks-cloud#240; the Sent proposal was approved on 2026-10-18).

## What Changes

- **Views beside the streams.** Sent (G then 5), Archive (G then 6), Spam
  (G then 7) and Trash (G then 8) sit under the four streams in the sidebar,
  after a quiet divider, and after the stream chips on phones. They are
  views, never a fifth stream: nothing is sorted into them.
- **Sent** lists every conversation the inbox wrote in, in any stream,
  archived or not, by the inbox's latest message. Rows lead with "To
  <name>" and say "Replied" when someone answered since. Mail you start
  stays out of the inbox until someone replies.
- **Delete (#) and Report spam (!)** with Undo, as Done has. In Archive the
  way back is Move to Inbox (E), in Spam Not spam (!), in Trash Restore (#):
  the key that sent it there brings it back.
- **Spam is safe to look at.** Remote images never load there and Show images
  is not offered, even for trusted senders, a banner says the conversation is
  in Spam, and Scout's suggestions, its drafts and the reply box are gone.
- **Server.** `conversations.list` takes `view=inbox|sent|archive|spam|trash`;
  `archived=true` stays as a deprecated alias of `view=archive`. Conversations
  keep `last_sent_at` and `spam`, worked out by `recount`, backfilled by the
  migration, with a partial index for each view's order. Summaries carry
  `trashed`, `spam`, `last_sent_at`, `sent_to` and `replied_since_sent`.

## Non-goals

- Emptying the trash or deleting forever: the provider empties its trash on
  its own schedule.
- Starred and Drafts views (follow-up campbooks-cloud#277).
- Mail outside the backfill window, and views of their own per provider
  folder or label.
- Searching `in:sent` (search keeps `in:inbox|archive|spam|trash|anywhere`).

## Impact

- Modules: `conversations` (server: list, recount, images; client: routes,
  sidebar, list rows, reader, shortcuts). The app shell's sidebar
  (`app/routes/workspace.tsx`) learns the views. No new module and no
  cross-module dependency.
- Specs: conversations (Reading conversations, Trash and spam, ADDED the
  inbox's latest message and spam images), inbox-ui (Streams, not people,
  ADDED views' actions, Sent rows and the Spam reader).
- Migration `20261022070000_conversations__mail_views` and four concurrent
  index migrations.
- API: `conversations.list` gains `view`; `ConversationSummary` gains five
  fields; `ImagesHeld` gains `spam`.
