# Proposal

## Why

An Outlook.com, Hotmail, Live, MSN or Microsoft 365 mailbox can be connected
(#315) and is read through Microsoft Graph (#316), but nothing a member does
in Campbooks reaches it: marking a message read, starring, archiving,
deleting, reporting spam or moving it to a stream is skipped as
`unsupported`, and a reply sent from a Microsoft inbox fails `not_connected`.
This change is the last slice of the Microsoft Graph epic: **write-back and
sending**, the same intents Gmail, Zoho and IMAP already carry, made the way
Graph makes them.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/317
(epic https://github.com/notacamp/campbooks-cloud/issues/309).

Modules touched: `connectors` only (the Graph client, the Microsoft adapter's
push, `request_changes`, `send_message`, the Microsoft reader). No new
module, no new cross-module dependency, no new crate, no migration, no new
setting, no new event, no new operation.

## What Changes

- **Write-back.** A Microsoft stream's run makes its pending changes before
  it reads, through the shared plan (`domain/push.rs`) like every adapter:
  read and flag (`PATCH /me/messages/{id}`, `isRead`, `flag.flagStatus`);
  archive and unarchive, delete and restore, spam and not spam, as moves
  between the folders the reader already knows (`POST …/move` to Archive,
  Deleted Items, Junk Email, the Inbox, or back where a deleted message
  was); streams as the mailbox's own `Campbooks/<Stream>` folder where it
  has one (a move), else a `Campbooks/<Stream>` Outlook category on a
  message kept in the Inbox, exactly the mapping #316 reads.
- **Never a permanent delete.** "Delete" is a move to Deleted Items, as Gmail
  trash is. The client has no call that deletes a message, empties a folder
  or deletes a folder. Messages are named by their Graph immutable ids
  (`Prefer: IdType="ImmutableId"`), so a move keeps the id and the index
  stays one row per message; an answer naming another id has the old one
  looked up by the next read.
- **Conflicts by `changeKey`.** Each message is read (`@odata.etag`) before it
  is written and patched with `If-Match`. A change Graph made since the last
  read to what the member's change touches wins (the change ends
  `superseded`, `changed_at_provider`, and Graph's state is shown), as an
  IMAP server's later change does. A 412 re-reads the message and applies
  that rule again; a second 412 is Graph's too.
- **Sending** through `POST /me/sendMail` with the message as MIME (the same
  RFC 5322 Gmail sends: `Message-ID`, `In-Reply-To`, `References`, Bcc in the
  headers), which Graph saves to Sent Items itself: Campbooks writes no copy.
  The Sent Items copy is found by its `internetMessageId` and indexed under
  its Graph id, so the next sync does not announce it again; until it is
  there, the attempt stays unclear and is only looked up again.
- **At most once.** Every sendMail is preceded by a committed attempt; an
  unclear one (a timeout, a 5xx) is looked up in Sent Items by its
  `internetMessageId` before anything else, and, when it is not found, is
  never sent again (`unknown_outcome`): Graph saves to Sent Items
  asynchronously, so a miss proves nothing.
- **From the verified mailbox only.** A send goes out only from the
  connection's own address, and only once that address is verified
  (`xms_edov`, #315); a document from any other address, or from an
  unverified one, is `rejected` before Graph is called. No `Sender` is ever
  set.
- **Limits.** Graph takes a MIME message of at most 4 MB (base64); a larger
  one fails `too_large` before any call (no upload session, see design).
  429 and 503 with `Retry-After` honour it, capped at an hour, for writes and
  sends; a run makes at most 50 messages' changes, and their requests are
  charged to the connection's budget.
- **The reader** shows a member's change still waiting as they left it, and
  emits again, with Graph's state, a message whose change was refused,
  overtaken or given up.
- **Off by default, still.** With `CAMPBOOKS_CONNECTORS__MICROSOFT__ENABLED`
  off, a Microsoft change is recorded `skipped` (`unsupported`), nothing is
  written or sent, and a send ends `not_connected`; no new setting can stop a
  server.

## Non-goals

- Upload sessions for messages over Graph's 4 MB MIME limit (they need a
  draft at Microsoft, see design).
- Label changes (`add_label` / `remove_label`) at Microsoft: a member's
  folder is a place, not a label, and Outlook categories other than stream
  marks are not written (they stay `unsupported`, as for IMAP).
- Creating `Campbooks/<Stream>` folders or Outlook master categories (the
  latter needs `MailboxSettings.ReadWrite`, which is not asked for).
- Focused / Other (`inferenceClassification`) as a stream.
- Checking an unverified Microsoft address by a test message, as IMAP does.
- Microsoft calendar.
