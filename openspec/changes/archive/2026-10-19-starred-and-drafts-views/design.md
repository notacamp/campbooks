## Context

The conversations module owns both lists. A conversation keeps `starred`
(the provider's star, shown as Pin) and `trashed`; `MailView` gives each
view fixed SQL fragments (filter, the time it pages on, whether pins come
first). Drafts are their own records (`conversations.drafts`) listed by
`conversations.list_drafts`, scope `conversations:send`.

No event is added or changed; the client refetches both lists on the
existing change feed. No AI is involved.

## Decisions

**Starred is a view, not a flag on the inbox.** `MailView::Starred` is
`c.starred AND NOT c.trashed`, in any stream, archived or not, newest first
by `last_message_at` and not pinned-first (everything in it is pinned, so
that would add nothing). Its cursor is the same `(time, id)` as Archive's.
Spam is binned, so a pinned conversation in spam or the trash leaves the
view and returns on Restore.

**An index of its own.** Because few conversations are pinned, the list
index (`conversations_list_idx … WHERE NOT trashed`) would have to walk every
unbinned conversation of the inbox to fill one page, and the list refetches on
every change. So Pinned pages over
`conversations_starred_list_idx (inbox_id, last_message_at DESC, id DESC)
WHERE starred AND NOT trashed` (migration
`20261023070100_conversations__starred_list_index`, built concurrently), as
Sent, Archive, Spam and Trash each have theirs. (The first draft of this change
relied on the list index; the review caught that it gets the selectivity
backwards.)

**Pinned, not Starred, in the UI.** The app says Pin, Unpin and "Pinned ·
subject" in all four locales, so the view is "Pinned" (pt-PT Fixadas, es-ES
Fijadas, fr Épinglées). The wire name stays `starred` because that is the
field and the provider's name.

**Keys follow history, the sidebar follows meaning.** G 5 to 8 are already
Sent, Archive, Spam and Trash in people's hands. Pinned and Drafts belong
after Sent, but renumbering would break those keys, so they take the keys
left, 9 and 0, and the sidebar order and the keys are allowed to differ.

**Drafts stays client-side over `list_drafts`.** The data is the caller's own
drafts, not conversations, and `list_drafts` already returns exactly that,
newest first. A server `view=drafts` would make `conversations.list` answer
two shapes and need a second scope. Instead `Place` is
`Stream | View | "drafts"` and the list functions (`belongsIn`,
`placeInLists`, `useConversations`) take a separate `ConversationPlace`, so
the type system stops them being called with Drafts. A small `DraftsScreen`
and `DraftsList` replace the conversation list; they are not a branch in
every conversation-list function.

**The cap is 50.** `list_drafts` returns at most 50 and has no cursor. The
list says "Your 50 newest drafts." only when it got exactly 50, which is the
only time older ones may be hidden. Drafts already on their way (scheduled or
sending) are left out: they can no longer be edited.

**Opening a draft opens exactly that draft.** A new message opens
`/compose?draft=` (`NewMessage`). An answer opens `/inbox/drafts/<conversation>?draft=<id>`:
the Drafts list stays on the left, Back and Escape return to it, and the
reader's reply slot opens its composer on that draft id, whatever its kind (the
same way the saved-draft chips pick a draft). `?reply=true&to=` keeps working
for Home's R. Delete and Undo are `useDeleteDraft`'s, unchanged.

**Live updates need nothing new.** The list is keyed by the
`conversations.list_drafts` operation, which the `conversations.draft`
record already invalidates on save, send and delete.

**Chip in view.** The active chip calls `scrollIntoView({ inline: "nearest",
block: "nearest" })` when the place changes, `behavior: "auto"` under
`prefers-reduced-motion`, else `"smooth"`. It covers streams and views alike.
