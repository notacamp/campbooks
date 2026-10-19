# Proposal

## Why

A member's mailbox should look the same in Campbooks and in the mail client
on their phone. Archive, spam, trash, read, star and Gmail labels already
sync both ways, but streams do not:

- Gmail has no Receipts tab, so a move to Receipts is `skipped`
  (`no_provider_equivalent`).
- Zoho has no tabs at all, so no stream move reaches Zoho. Zoho's own user
  labels are skipped too, although Zoho has them.
- Scout's automatic sort is never sent to the provider. Only taught rules
  and hand moves send `move_to_stream`, so Gmail's tabs disagree with
  Campbooks for most mail.
- A tab change made in Gmail is stored (`categories`) but never moves the
  conversation in Campbooks.

The owner's rule (2026-10-19): what the provider supports natively syncs
natively; what it does not is carried by a system of conventionally named
labels in the provider. The owner also chose to mirror **all** sorting
(Scout's automatic sort, taught rules and hand moves), so the mail client
always matches Campbooks.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/292.

## What Changes

- **Stream marks.** A stream the provider has no native place for is
  carried by a provider label named `Campbooks/<Stream>`: at Gmail,
  `Campbooks/Receipts`; at Zoho, `Campbooks/Updates`, `Campbooks/Receipts`
  and `Campbooks/Newsletters` (no mark means Primary). The connector
  creates the label at the provider the first time it needs it, and finds
  it again by name. At most one stream mark is on a message.
- **Moves reach the provider for every stream.** `move_to_stream` writes
  the native place where there is one (Gmail's categories) and the stream
  mark where there is not, and takes every other stream mark off. A move to
  Receipts at Gmail puts the message in the Updates tab and adds
  `Campbooks/Receipts`.
- **Every adapter reports a provider stream** (new optional field
  `provider_stream` in `MessageChangeBatch` v1, additive): the stream the
  message is in at the provider, read from its stream mark first, then its
  native place (Gmail: Updates, Promotions → `newsletters`, Social and
  Forums → none, otherwise `primary`; Zoho: `primary` without a mark).
  Stream marks are never carried as labels, so they never show as member
  labels.
- **All sorting is mirrored.** When Scout's sort places a conversation, and
  when a new inbound message joins an already sorted conversation,
  conversations requests `move_to_stream` for each inbound message whose
  provider stream differs from the conversation's. Rules and hand moves
  already request it.
- **Moves made in the mail client come back.** When a known message's
  provider stream changes (a Gmail tab changed, or a `Campbooks/*` mark
  added or removed after the message arrived), the conversation moves to
  that stream with stream source `member`, and its other inbound messages
  are moved at the provider too. The provider's own sort when the message
  arrives never moves anything: Campbooks sorts new mail.
- **Zoho user labels sync natively**: read with each message, added and
  removed with Zoho's `applyLabel` / `removeLabel`. This needs Zoho's labels
  scope (`ZohoMail.tags.ALL`); a Zoho connection granted before this change
  keeps working without labels and stream marks until the member reconnects.
- **No echo, no loop.** Our own writes leave the adapter's index holding
  what we set, as today. A requested stream change that is already as asked
  at the provider makes no write.
- **Migrations:** a `connectors.zoho_labels` cache, and
  `conversations.messages.provider_stream`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `connectors`: *Gmail messages adapter*, *Outbound changes to Gmail*,
  *Zoho messages adapter*, *Outbound changes to Zoho*; adds *Stream marks
  at the provider*.
- `conversations`: *Applying change batches*, *Streams from accepted
  answers*, *Tidying conversations*.

## Impact

- Code: `crates/modules/connectors` (Gmail and Zoho adapters, labels APIs,
  outbound mapping) and `crates/modules/conversations` (apply_changes,
  decide_message's placement, a shared stream-mirror helper).
- Contract: `message-change-batch.v1` gains an optional `provider_stream`
  (additive; older batches without it change nothing).
  `conversations.changes_requested.v1` is unchanged.
- Database: two migrations (`connectors`, `conversations`).
- Gmail quota: `labels.create` (5 units) once per mailbox; one `modify`
  (5) or a share of a `batchModify` (50) per message whose tab differs.
- Zoho: one more OAuth scope. Existing Zoho connections must reconnect once
  to sync labels and streams.
- No new endpoints, settings or UI.

## Non-goals

- Relabelling existing mail. Only sorting from this change on is mirrored.
- Learning from moves made in the mail client (they move the conversation
  but record no correction for Scout). Possible follow-up.
- Creating provider labels for labels a member makes in Campbooks
  (`openspec/decisions-needed.md`, "Labels made in Campbooks").
- Localised mark names. `Campbooks/<Stream>` is a protocol name, the same
  in every language, so the connector can find it again.
- Showing the member that a Zoho connection needs reconnecting for labels.
  It is a follow-up on the connections screen.
- Snooze, Done and other Campbooks-only states. They are not mail state.
