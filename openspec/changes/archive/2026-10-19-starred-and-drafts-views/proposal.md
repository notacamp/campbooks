## Why

Sent, Archive, Spam and Trash sit beside the streams (campbooks-cloud#240),
but two lists the owner asked for are missing: what you pinned, wherever it
is, and the drafts you left half written. The pin is already a conversation
flag and `conversations.list_drafts` already lists the caller's drafts, so
neither needs new data (campbooks-cloud#277). On phones the chip row stays
scrolled to the start, so the chip of the place you are in can be off screen
(campbooks-cloud#279).

## What Changes

- **Pinned** (G then 9) lists the conversations you pinned, in any stream,
  archived or not, outside the trash, newest first. The app calls the star
  "Pin" everywhere, so the view is "Pinned"; the API keeps the name
  `starred`: `conversations.list` takes `view=starred`.
- **Drafts** (G then 0) lists your saved drafts, newest first, at most 50,
  from `conversations.list_drafts`. A row says who the draft is to, its
  subject, a line of its words and when it was saved. Opening a row opens the
  composer where the draft was left; Delete draft has Undo. It is a place
  beside the views whose list is drafts, not conversations.
- **Sidebar order follows meaning, keys follow history.** The order is Sent,
  Pinned, Drafts, Archive, Spam, Trash. G 5 to 8 keep Sent, Archive, Spam
  and Trash, so nobody's habit breaks; the two new places take 9 and 0.
- **Phones:** when the place changes, its chip scrolls into view (no
  animation under reduced motion).

## Non-goals

- A migration or a new index (see design).
- Paging Drafts past 50, or drafts of other members.
- Counts on the new places.
- Starring from the Drafts list, or searching drafts.

## Impact

- Modules: `conversations` (server: the `starred` view; client: places,
  sidebar, chips, the Drafts screen). The app shell's sidebar learns the
  places. No new module, no cross-module dependency, no event.
- Specs: conversations (Reading conversations), inbox-ui (Streams, not
  people; ADDED Drafts and the chip in view).
- API: `conversations.list` gains `view=starred`. No new scope: Drafts reads
  with `conversations:send`, as the composer already does.
