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
`in_inbox` columns, and a new `messages.inbox_before_spam`; a conversation
counts as trashed when every message is in the trash or spam (`recount`,
unchanged). Trash sets `in_trash` and clears `in_spam`, leaving `in_inbox`
as it is; untrash clears `in_trash` only, so the message is back where it
was. Spam remembers `in_inbox` in `inbox_before_spam`, then sets `in_spam`
and clears `in_inbox` and `in_trash` on every message of the conversation, as
a provider reports a thread; not spam clears `in_spam` and sets `in_inbox`
back to what was remembered, and never for a message the member sent. `conversation_changed` v1 carries the change as `messages`
(its existing kind for moves in and out of the trash); `changes_requested`
v1 carries one item per message that changed, with the new kinds. Schema
`connectors`: `outbound_changes.kind` gains the kinds (a widened CHECK added
`NOT VALID`, validated in the next migration), and `outbound_changes.restore_to`
keeps, on a change that put a message in the trash or spam, the provider's
location tokens of where it was (Gmail: `INBOX` or nothing; Zoho: its folder,
its kind, and whether it was archived there). Ids and labels only.

## Access

Every operation goes through `flags::apply`, which locks the conversation
through the access catalogue (`conversations.conversation`, `write`) before
it reads or changes anything: a caller who cannot see it gets 404, a reader
403 `inbox_read_only`. Write-back can only touch the messages listed in the
event, which the module took from the conversation it locked, and the
connector makes them only on the stream that feeds the event's inbox, exactly
as for archive.

## Undo puts it back where it was

The coordinator decided (review of the first version): untrash and not spam
restore the message's previous place, never "always the inbox", and local
state matches the provider's.

| | **a. Remember the place on both sides (chosen)** | b. Always back to the inbox | c. Ask the provider where it was |
| --- | --- | --- | --- |
| Archived mail reported by mistake | back to the archive | lands in the inbox | Gmail can (untrash), Zoho cannot |
| Own sent mail | stays in Sent | moved into the inbox | depends |
| Cost | one column per side | none | not available for spam |

Gmail's `messages.untrash` restores the labels itself, so Gmail needs no
memory for the trash. For spam (both providers) and for Zoho's trash, the
push records where the message was on the change that moved it
(`restore_to`), and the change that takes it out reads the latest one.
Conversations keeps `inbox_before_spam` for its own `in_inbox`.

## Provider mapping

- **Gmail**: trash with `users.messages.trash`, untrash with
  `users.messages.untrash`; spam adds `SPAM` and removes `INBOX` (and
  `TRASH`), not spam removes `SPAM` and adds `INBOX` only when `restore_to`
  says it was there. `TRASH` and `SPAM` join the labels a change may write. A
  change that would bring a message held in the trash or spam back to the
  inbox is still skipped (`in_trash`), unless the change itself takes it out
  of the trash or spam.
- **Zoho**: trash is `moveMessage` to the Trash folder from the account's
  folder listing; spam is `moveToSpam`, Zoho's documented report. Out of
  either, `moveMessage` back to the remembered folder, then `archiveMails`
  if it was archived there. Zoho also documents a "not spam" mode, but under
  two names (`moveNotSpam` in its table, `markNotSpam` in its sample); until
  it is checked against a real account it is not used, so not spam at Zoho is
  a folder move and does not tell Zoho's filter. A Trash folder the listing
  did not show, or a message with nowhere remembered, is never guessed:
  `skipped` (`not_movable`), and the provider's state is emitted again. The
  index keeps the tokens written, so the next listing emits nothing.

## AI use

None.
