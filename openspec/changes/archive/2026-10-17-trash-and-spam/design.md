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
`in_inbox` columns; a conversation counts as trashed when every message is
in the trash or spam (`recount`, unchanged). Trash sets `in_trash` and clears
`in_spam`, leaving `in_inbox` as it is; untrash clears `in_trash` only, so
the message is back where it was. Spam sets `in_spam` and clears `in_inbox`
and `in_trash` on every message of the conversation, as a provider reports a
thread; not spam clears `in_spam` and sets `in_inbox` for mail the inbox
received (`direction = 'inbound'`), never for the member's own mail.
`conversation_changed` v1 carries the change as `messages` (its existing kind
for moves in and out of the trash); `changes_requested` v1 carries one item
per message that changed, with the new kinds; `untrash` and `unspam` carry the
message's direction (`inbound`). Schema `connectors`: `outbound_changes.kind`
gains the kinds (a widened CHECK added `NOT VALID`, validated in the next
migration), and `outbound_changes.restore_to` keeps, on a change that put a
message in a Zoho trash, Zoho's location tokens of where it was (its folder,
the folder's kind, and whether it was archived there); a sync clears it once
the index no longer has the message in the trash. Ids and tokens only.

## Access

Every operation goes through `flags::apply`, which locks the conversation
through the access catalogue (`conversations.conversation`, `write`) before
it reads or changes anything: a caller who cannot see it gets 404, a reader
403 `inbox_read_only`. Write-back can only touch the messages listed in the
event, which the module took from the conversation it locked, and the
connector makes them only on the stream that feeds the event's inbox, exactly
as for archive.

## Taking mail out again

The coordinator decided (second review): remember as little as possible.

| | **a. Trash restores; not spam returns received mail to the inbox (chosen)** | b. Remember every place, both providers | c. Always back to the inbox |
| --- | --- | --- | --- |
| Spam a provider flagged itself, or reported before this ships | to the inbox | nothing remembered: stays out | to the inbox |
| Mail reported while archived | to the inbox (as Gmail's own "Not spam") | back to the archive | to the inbox |
| The member's own mail | never in the inbox | stays where it was | in the inbox |
| State kept | Zoho's trash folder only | a column per side, kept in step | none |

- **Spam**: nothing is remembered. Not spam puts mail the inbox received back
  in the inbox, at the provider and here, as Gmail's own button does; the
  member's own mail is only taken out of spam (at Zoho, which only moves it,
  that is `not_movable`).
- **Trash**: Gmail's `messages.untrash` puts back the labels the message had,
  so nothing is remembered for Gmail. At Zoho the push keeps the folder the
  message was in on the change that trashed it. Untrash moves it back there
  only when that is the latest trash or spam change made to the message,
  the message is still in the trash per the index (a sync that saw it leave
  clears the record), the account's folder listing still shows that folder,
  and it is neither the Trash nor the Spam folder. Otherwise received mail
  goes to the Inbox folder and anything else is `skipped` (`not_movable`),
  never `message_gone`. Locally, trash and untrash leave `in_inbox` alone.

## Provider mapping

- **Gmail**: trash with `users.messages.trash`, untrash with
  `users.messages.untrash`; spam adds `SPAM` and removes `INBOX` (and
  `TRASH`); not spam removes `SPAM` and adds `INBOX` for received mail only.
  `INBOX` is added by an explicit not-spam change and nothing else. `TRASH`
  and `SPAM` join the labels a change may write. A change that would bring a
  message held in the trash or spam back to the inbox is still skipped
  (`in_trash`), unless the change itself takes it out of the trash or spam.
- **Zoho**: trash is `moveMessage` to the Trash folder from the account's
  folder listing; spam is `moveToSpam`, Zoho's documented report. Out of the
  trash, as above; out of spam, `moveMessage` to the Inbox folder for received
  mail. Zoho also documents a "not spam" mode, but under two names
  (`moveNotSpam` in its table, `markNotSpam` in its sample); until it is
  checked against a real account it is not used, so not spam at Zoho is a
  folder move and does not tell Zoho's filter. A Trash folder the listing did
  not show is never guessed: `skipped` (`not_movable`), and the provider's
  state is emitted again. The index keeps the tokens written, so the next
  listing emits nothing.
- **Unverified**: Gmail's `messages.untrash` restoring the earlier labels, and
  what Zoho's `moveToSpam` does with an archived message, are taken from the
  providers' documentation and have not been checked against a live account.

## AI use

None.
