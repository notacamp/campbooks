## Context

The conversations module owns everything here, in its own Postgres schema
(`conversations`). A conversation already keeps `archived_at` (no message in
the inbox) and `trashed` (every message in the trash or spam), both derived
from its messages by `recount` whenever a message changes. Messages keep
`direction`, `in_inbox`, `in_spam` and `in_trash`. Nothing told spam from
trash at the conversation level, and nothing said when the inbox last wrote.

No event is added or changed: `conversations.conversation_changed` keeps its
state (`archived`, `trashed`); the client refetches lists on it as before.
No AI is involved.

## Decisions

### Views, not streams

A stream is where Campbooks sorts a conversation (exactly one, by the decide
pass, a rule or a member), and the streams together are the inbox. Sent,
Archive, Spam and Trash are not places mail is sorted into: they are other
ways to look at the same conversations, and a conversation can be in several
at once (an archived thread you wrote in is in Sent and in Archive). Making
them streams would mean a fifth stream the decide pass could pick, moving
mail between them, and unread counts that mean nothing for Sent. So they are
a `view` of `conversations.list`, the sidebar draws them apart from the
streams, and they have no counts.

The route keeps `/inbox/$stream` and its param name: the param holds a
stream or a view, and anything else still goes to Primary. Renaming it would
touch every link to a stream for no behaviour.

### Stored columns, not `EXISTS` per page

Sent needs "the inbox wrote here", ordered by when; Spam and Trash need why
a conversation is binned. Both can be worked out from the messages on every
list (`EXISTS` over outbound messages, `max(sent_at)` in the sort), but then
no index can serve the order: every page sorts every candidate conversation.
Instead `recount` keeps two columns on the conversation, in the same
statement that keeps `archived_at` and `trashed`:

- `last_sent_at`: the newest `sent_at` of its outbound messages in neither
  spam nor the trash (NULL when none);
- `spam`: `trashed`, and at least one message in spam and not in the trash
  (so a binned conversation is in Spam or in Trash, never both; a check
  constraint keeps `spam` false when not `trashed`).

Each view then is a keyset page over one partial index, `(inbox_id, <time>
DESC, id DESC)` with the view's filter as the index predicate: Sent on
`last_sent_at`, Spam, Trash and Archive on `last_message_at`. The indexes are
built concurrently, one per migration, like the pinned list indexes. The
migration backfills both columns with one `UPDATE` from the messages (the
same rule as `recount`; the table is small in production) and is idempotent.

Every write that changes a message's flags or adds a message (sync, the
member's flag operations, rules, sends recorded through sync) already ends in
`recount`, so no other write path sets the columns.

### One query, the view's fragments

`list_conversations` builds one SQL statement from a fixed table: each view
gives its `WHERE` condition, the column its pages are keyed on, and whether
pins come first (the inbox only). No user input reaches the SQL text; every
value is bound. The summary's columns (`SUMMARY_SELECT`) are shared by the
list, the `ids` list and the single summary. These queries moved off the
`query_as!` macros, so the offline cache lost their entries rather than
gaining new ones.

The cursor stays `(pinned, time, id)`: views never carry a pin, and Sent's
time is `last_sent_at`. `archived=true` reads as `view=archive`; with another
view it is 400 `invalid_view`. `ids` lists ignore the view, as they ignore the
stream.

### Summaries say where a conversation is

`ConversationSummary` gains `trashed` and `spam` (like `archived`, so the
client can place it and the reader can show the banner and the way back),
`last_sent_at`, `sent_to` (to, then cc, of the inbox's latest message outside
spam and the trash) for "To Marta Ferreira and 2 others", and
`replied_since_sent` (an inbound message outside spam and the trash newer than
`last_sent_at`) for "Replied".

### Spam is never trusted

A conversation lands in Spam because it is unwanted or hostile. Trusted-sender
images exist to save a click on mail the member wants; in spam the sender's
identity is exactly what is in doubt, and a remote image is a read receipt
for the spammer. So the one place that decides images (`rules::images`) says
`held: spam` for a message in spam or in a conversation reported as spam,
before any rule is read, and the body keeps the images held back even when
`show_images=true` is asked for. The client offers neither Show images nor
"Always show images from …" there, makes links inert as for suspicious mail,
and hides the reply box, Scout's suggestions and its drafts in Spam and in
Trash: answering or sorting binned mail is not what anyone is there to do.
Not spam and Restore bring all of it back.

### Keys

Delete is `#` and Report spam `!` (operation-keyed: `conversations.trash`,
`conversations.report_spam`, ADR 0013), both free in the registry. In the
trash `#` restores and in spam `!` says it is not spam, so the key that sent a
conversation somewhere takes it back; E moves an archived conversation back
to the inbox. One button style per toolbar: the primary action keeps the
Done button's style, Delete and Report spam are icon buttons beside Pin and
Mark read, and in Spam and Trash the way back is the banner's one button.

## Scopes

`conversations.list` keeps `conversations:read`; trash, untrash, report spam
and not spam keep `conversations:write`. No endpoint is added.

## Risks

- Not spam puts received mail back in the inbox even if it was archived
  before it was reported (the provider's own "Not spam", kept on the
  server). So the client's Undo of Report spam on an archived conversation
  is Not spam then archive, one Undo entry with one toast, and puts it back
  in the archive; the plain Not spam still means "back to the inbox".
- A message deleted at the provider before this migration but still synced
  in the trash keeps its conversation in Trash until the provider empties it.
