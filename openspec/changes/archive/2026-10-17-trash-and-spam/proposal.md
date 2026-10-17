## Why

Home's "Report and delete" (campbooks-cloud#164, prototype v6) and the smart
rule that follows it ("Never see emails like this again?",
campbooks-cloud#169) need Campbooks to put a conversation in the trash or
report it as spam, at the provider too. Next can archive, star, label and
move between streams, but has no trash and no spam: no operation, no change
kind in `conversations.changes_requested` v1, no write-back in either adapter.
The coordinator assigned all of this plumbing to #169; #166 only adds a
catalogue entry that calls the operation.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/169.

Modules touched:

- **`conversations`**: four operations (`trash`, `untrash`, `report_spam`,
  `not_spam`) built on the existing flag machinery; their need settles as
  "Spam and trash need nothing" already says.
- **`connectors`**: the four change kinds reach Gmail (labels `TRASH` and
  `SPAM`) and Zoho (moves to the account's Trash, Spam and Inbox folders).
- **Contract**: `conversations.changes_requested` v1 gains the kinds `trash`,
  `untrash`, `spam` and `unspam` (additive; an older consumer reads them as
  unknown and skips them).

No new module and no new cross-module dependency.

## What Changes

- `POST …/conversations/{id}/trash` (`conversations.trash`, undo
  `conversations.untrash`) and `/untrash`; `POST …/conversations/{id}/spam`
  (`conversations.report_spam`, undo `conversations.not_spam`) and
  `/not-spam`. Scope `conversations:write`, the catalogue's `write` on the
  conversation (a reader gets 403 `inbox_read_only`, anyone who cannot see it
  404). Each changes only what is not already so, publishes
  `conversation_changed` and `changes_requested`, and works out the need again
  (trashed or reported mail needs nothing).
- Report means spam: Gmail and Zoho both treat a move to spam as the member's
  report, which is what "Report and delete" asks for. Plain trash stays for
  deleting without reporting. Trash and spam exclude each other.
- **Undo puts a message back where it was** (coordinator's decision), never
  always in the inbox: untrash leaves the inbox flag as it was before the
  trash; not spam returns a message to the inbox only if it was there before
  it was reported (remembered per message), never the member's own sent mail.
- Write-back: Gmail uses `messages.trash` / `messages.untrash` (Gmail puts
  back the labels the message had); spam adds `SPAM` and removes `INBOX`, not
  spam removes `SPAM` and adds `INBOX` only if the message was in the inbox.
  Zoho trash moves to the account's Trash folder and spam uses Zoho's report
  (`moveToSpam`); both remember the folder (and whether archived there), and
  untrash / not spam move the message back to it. Zoho's "not spam" mode is
  documented two ways and is not used: taking a message out of spam at Zoho is
  a folder move that does not tell Zoho's filter. A folder the listing did not
  show, or nowhere remembered, is never guessed (`not_movable`).
- `connectors.outbound_changes` accepts the four kinds and keeps `restore_to`
  (where a message was before the trash or spam); `conversations.messages`
  keeps `inbox_before_spam`.

## Non-goals

- Deleting mail for good, or emptying the trash.
- The Home button and its toast (#166 adds the catalogue entry; this change
  gives it the operation and its undo).
- Smart rules that trash or report (the `rule-offers` change on top of this one).
