# Design

## Report = spam, delete = trash

| | **a. Report moves to spam; trash is separate (chosen)** | b. Report trashes | c. Report calls a provider "report phishing" API |
| --- | --- | --- | --- |
| Tells the provider | yes: Gmail and Zoho learn from a move to spam | no | Gmail has no such API; Zoho none documented |
| Undo | not spam, back to the inbox | untrash | none |
| Reuses | one flag pair, one change kind pair | the trash pair | new provider calls per adapter |

No spam change kind or operation existed, so this change adds both pairs, as
the coordinator asked: `spam`/`unspam` for "Report and delete", `trash`/
`untrash` for deleting without a report (and for smart rules that just delete).

## Data ownership and events

Schema `conversations`: the existing `messages.in_trash`, `in_spam` and
`in_inbox` columns; a conversation counts as trashed when every message is in
the trash or spam (`recount`, unchanged). Trash sets `in_trash`; spam sets
`in_spam` and clears `in_inbox` on every message of the conversation, as a
provider reports a thread; not spam does the reverse; untrash clears
`in_trash` only. `conversation_changed` v1 carries the change as `messages`
(its existing kind for moves in and out of the trash); `changes_requested`
v1 carries one item per message that changed, with the new kinds. Schema
`connectors`: `outbound_changes.kind` gains the kinds (a widened CHECK added
`NOT VALID`, validated in the next migration).

## Access

Every operation goes through `flags::apply`, which locks the conversation
through the access catalogue (`conversations.conversation`, `write`) before
it reads or changes anything: a caller who cannot see it gets 404, a reader
403 `inbox_read_only`. Write-back can only touch the messages listed in the
event, which the module took from the conversation it locked, and the
connector makes them only on the stream that feeds the event's inbox, exactly
as for archive.

## Provider mapping

- **Gmail**: `TRASH` and `SPAM` are written like `INBOX` (they join the
  labels a change may write). A change that would bring a message held in the
  trash or spam back to the inbox is still skipped (`in_trash`), unless the
  change itself takes it out of the trash or spam (untrash, not spam).
- **Zoho**: `moveMessage` to the Trash or Spam folder from the account's
  folder listing; out of either, `moveMessage` back to the Inbox folder, only
  for a message actually there. The index keeps the tokens written, so the
  next listing emits nothing.

## AI use

None.
