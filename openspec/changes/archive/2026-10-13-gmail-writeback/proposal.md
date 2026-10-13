# Proposal

## Why

What a member does in Campbooks has to reach their mailbox. Today a member
can mark a conversation read, star it, archive it, label it or move it to
another stream, and the `conversations` module publishes
`conversations.changes_requested` for each of these
(`contracts/conversations.changes_requested.v1.schema.json`). Nothing
consumes it yet, so Gmail never hears about it: the same mail is still
unread and in the inbox on the member's phone, and the next change at Gmail
can quietly undo what they did in Campbooks.

ADR 0010 gives this work to the `connectors` module: domain modules emit an
**outbound change**, and the adapter performs it with retries and conflict
checks. The archived `add-connectors-module` design sketched it; this change
designs it in full and builds it for Gmail.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/66.

## What Changes

- **Module `connectors` consumes `conversations.changes_requested`**
  (`connectors.request_changes`). Each item becomes an outbound change
  for the inbox's Gmail stream: read and unread (`UNREAD`), star and
  unstar (`STARRED`), archive and unarchive (`INBOX`), a provider label
  added or taken off (its Gmail label id), and a move to a stream that has
  a Gmail category (Primary, Updates, Newsletters → Promotions). Each is
  recorded in a new table, `connectors.outbound_changes`, stamped with
  Gmail's history id at that moment, and the stream is asked to run now.
- **Every sync run pushes before it pulls.** The run applies the stream's
  pending changes first: it merges them per message and label (the latest
  wins), checks Gmail's history for a later change to the same label, makes
  the rest with `users.messages.modify` (or `users.messages.batchModify` for
  a group of more than ten messages with the same change), and records the
  outcome of each. Throttling (429), outages (5xx) and refused tokens use
  the run's existing backoff and the shared Google quota budget; a change
  Gmail refuses for good (or that keeps failing) is recorded as `failed`
  with a reason code, never with content.
- **No echo.** Changes from Gmail never come back as intents
  (conversations publishes `changes_requested` only for members). A write
  advances the adapter's index to the state it set, so when Gmail's history
  reports the write the next run finds nothing new and emits nothing. While
  a change waits, the upserts a run emits for that message show it as the
  member left it.
- **Last writer wins, in Gmail's order.** A member's change is placed at the
  history id stamped when it was recorded. If Gmail's history shows a change
  to the same label of the same message after that point (someone changed
  it in Gmail later), Gmail's change wins: the write is dropped as
  `superseded` and Campbooks is told the message's state again. A change
  made in Gmail earlier than the member's is overwritten. Campbooks' own
  writes are recognised by the history window of the run that made them.
- **Migration** `connectors__outbound_changes`: the new table (row-level
  security like every workspace table) and a `reemit` flag on
  `connectors.gmail_messages`.
- **Gmail client:** `messages.modify`, `messages.batchModify`; history
  records now carry their id and label ids.
- **Fake Google** (tests): modify and batchModify with history records, a
  message that refuses changes, 5xx answers.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `connectors`: adds the requirement *Outbound changes to Gmail*.

## Impact

- Code: `crates/modules/connectors` only (a new consumer and a push step
  in the existing sync run). No change to `conversations`, its contract, or
  the kernel.
- Database: one migration in schema `connectors`.
- Gmail quota: 1 unit to record a change, 5 per message written (50 per
  batch of more than ten), 2 per history page read for the conflict check,
  1 after writing. The run takes them from the shared provider budget.
- No new crates, settings, endpoints or scopes.

## Non-goals

- **Creating Gmail labels for labels made in Campbooks.** An `add_label`
  without a provider key is recorded as `skipped`
  (`label_not_at_provider`): the intent carries no label name, and putting
  new labels into a member's Gmail is a product decision. See
  `openspec/decisions-needed.md`.
- **The Receipts stream at Gmail.** Gmail has no Receipts category; a move
  there is recorded as `skipped` (`no_provider_equivalent`).
- Trash, spam, delete and send: not intents yet.
- Showing failed write-backs to the member (a count on the connection, a
  notice): a follow-up once the inbox screens can show it.
- Other providers (Graph, IMAP): the outbound table and the run's push
  step are provider-neutral, the Gmail code is not.
