# Design

## Context

Outbound changes: `conversations.changes_requested` → `connectors.request_changes`
→ `connectors.outbound_changes` → pushed by the stream's next run
(`domain/outbound.rs` `map` / `map_zoho`, `gmail/write.rs`, `zoho/write.rs`).
Each request is recorded and MAC'd by conversations (`domain/conversation.rs`
`publish_request`) and checked through the outbound-requests port before
the connector acts. Inbound: adapters emit `MessageChangeBatch` v1, applied
by `conversations.apply_changes`. Today categories are written on insert
only and nothing reads them.

## Decisions

### 1. Native first, a stream mark otherwise

| Stream | Gmail | Zoho |
|---|---|---|
| Primary | `CATEGORY_PERSONAL` | no mark |
| Updates | `CATEGORY_UPDATES` | `Campbooks/Updates` |
| Receipts | `CATEGORY_UPDATES` + `Campbooks/Receipts` | `Campbooks/Receipts` |
| Newsletters | `CATEGORY_PROMOTIONS` | `Campbooks/Newsletters` |

A move writes the row of the target stream and takes off every other
category (Gmail) and every other stream mark. Receipts sits in Gmail's
Updates tab because that is where Gmail itself files receipts. Primary has
no mark at Zoho, so the mark-free inbox stays the member's normal inbox.

*Alternatives.* (a) `Campbooks/<Stream>` for every stream at every provider.
This is uniform, but it duplicates Gmail's tabs, which the owner ruled out
("native first"). (b) A separate folder per stream at Zoho. A folder move
fights archive and trash, which are already folder moves at Zoho, and one
message can carry several labels but only one folder. Labels win.

### 2. Marks are a protocol, found by name

Fixed English names `Campbooks/Primary` (reserved, never written),
`Campbooks/Updates`, `Campbooks/Receipts`, `Campbooks/Newsletters`, in one
constant set in `connectors` (`domain/marks.rs`). They are matched by exact
name, case-insensitive, against the provider's label listing (Gmail
`gmail_labels`, Zoho `zoho_labels`). When a write needs one that does not
exist, the push creates it first (`labels.create` at Gmail; Zoho's create
label call), refreshes the cache and goes on. If the member deletes the
label, the next write that needs it creates it again. If they rename it, it
stops being a mark and becomes an ordinary user label. On Gmail, the `/`
nests them under a "Campbooks" parent.

### 3. One field across the contract: `provider_stream`

The adapter, not conversations, knows which label ids are marks. Each
upsert of an inbound message (not Sent, not drafts) carries
`provider_stream: "primary" | "updates" | "receipts" | "newsletters" | null`:

- **Gmail:** a mark if present (if several, none: `null`); otherwise
  `CATEGORY_UPDATES` → `updates`, `CATEGORY_PROMOTIONS` → `newsletters`,
  `CATEGORY_SOCIAL` / `CATEGORY_FORUMS` → `null`, else `primary`.
- **Zoho:** a mark if present (several → `null`), else `primary`.

Marks are removed from `labels` (never member labels). The field is
optional in the v1 schema. An upsert without it (old blob, provider
without streams) leaves the stored value untouched and moves nothing.

### 4. Conversations: the stored provider stream drives both directions

Migration: `conversations.messages.provider_stream text NULL`, with a check
for the four names.

- **Insert** (new message): store `provider_stream`. Nothing moves, because
  the decide pass sorts new mail.
- **Update** (known message): if the incoming value is non-null and differs
  from the stored one, it is a **provider move**:
  1. Store it.
  2. If it differs from the conversation's stream, move the conversation
     there with stream source `member` and publish `conversation_changed`
     (origin `provider`, changes `[stream]`).
  3. Request `move_to_stream` (to the conversation's stream, now) for the
     conversation's *other* inbound messages whose stored provider stream
     differs, and for this message too when its markers are inconsistent
     (the incoming value came from a category change while a mark is still
     on, as reported by the adapter; see 5).

  A null incoming value, or an equal one, only updates the stored column.
- **Mirror (outbound)**: a helper `streams::mirror(tx, conversation)`
  requests `move_to_stream` for every inbound message of the conversation
  whose stored `provider_stream` is not null-equal to the conversation's
  stream. A `null` stored value counts as different, so Social or Forums
  mail is moved into its Campbooks stream. It is called:
  - by `decide_message` after its placement (`sort_first_inbound`), whether
    the stream changed or not (a Primary placement must take Gmail's
    Updates tab off too);
  - by `apply_changes` when an inbound message is inserted into a
    conversation whose stream source is not `default` (already sorted);
  - by the provider-move path above.
  `move_to_stream` (hand moves) and rules keep their own requests.

All requests go through `conversation::request`, so they are recorded,
MAC'd and pass the standing check like a member's. `RequestOrigin` stays
`user` ("a member, automation or the assistant"). The MAC does not cover
origin, and the contract keeps `const: "user"`.

**Deviation from *Tidying conversations*:** "Changes that came from the
provider SHALL never produce `changes_requested`" gains one exception: a
provider move requests `move_to_stream` for the conversation's *other*
messages and for stale marks. This cannot loop, for three reasons:
(i) requests target the stream the provider just reported, so the message
that reported it is already there; (ii) the connector makes no write for a
change that is already in place (decision 6); (iii) a write it does make
leaves the index holding what it set, so it produces no new batch.

### 5. Inconsistent markers at Gmail

A member who drags a `Campbooks/Receipts` message to the Promotions tab
leaves the mark on. The adapter emits `provider_stream` from the mark
(`receipts`), so the tab change would be lost. Rule: when an incremental
read sees a **category** change on a message that carries a mark and no
mark change in the same read, the category wins. The adapter emits the
category's stream with `stream_mark_stale: true`, an optional boolean
beside `provider_stream`. Conversations then requests `move_to_stream` for
that message too, which takes the stale mark off. In steady state (a mark
and `CATEGORY_UPDATES`, as we write Receipts) the mark wins.

### 6. Already in place, no write

`map` / `map_zoho` stay pure. The push compares the change's labels with
the message's indexed labels (`apply_labels(current, change) == current`)
and records the change `skipped` (`already_set`) without a provider call.
This matters because mirroring requests a lot of moves that are already
true.

### 7. Scout's sort fights Gmail's tabs: accepted

With all sorting mirrored, Gmail's tab is overwritten by Scout's stream for
new mail. A wrong Scout sort is therefore wrong in Gmail too. The owner
accepted this (2026-10-19) in exchange for one consistent mailbox. The
member's fix, in either place, wins and sticks (stream source `member`).

### 8. Zoho labels

- **Scope:** `ZohoMail.tags.ALL`, added to the connect scopes in
  `zoho/mod.rs` and `provider.rs`. Verify the exact scope name and endpoints
  against Zoho's Mail API docs (labels API: list, create; `updatemessage`
  modes `applyLabel` / `removeLabel` with `labelId`) before coding.
- **Cache:** `connectors.zoho_labels (stream_id, label_id, name,
  refreshed_at)`, RLS like `gmail_labels`, refreshed when a listed message
  carries an unknown label id and at each sweep.
- **Read:** the message listing's label ids (field name per the docs) become
  tokens `label:<id>`. They are compared and indexed like the other tokens,
  so a label change re-emits the message without a download.
- **Write:** `writable` admits `label:<id>`. `AddLabel` / `RemoveLabel` with
  a provider key Zoho lists map to `label:<id>` on or off, and `after()`
  stores them.
- **Old grant:** if Zoho refuses the labels calls for the scope (or the
  stored grant lacks it), the stream reads without labels or marks (no
  `provider_stream`) and records label and stream changes `skipped`
  (`scope_missing`). Nothing else breaks.

## Data ownership and events

- `connectors` owns `zoho_labels`, the mark names, mark creation and the
  `provider_stream` derivation. It consumes `conversations.changes_requested`
  (unchanged) and publishes batches (contract gains `provider_stream`,
  `stream_mark_stale`).
- `conversations` owns `messages.provider_stream`, the mirror helper and the
  provider-move rule. It publishes `conversation_changed` and
  `changes_requested` (both unchanged in shape).
- No new endpoints, so there are no new API scopes. Provider OAuth gains
  `ZohoMail.tags.ALL`.
- No AI use.

## Quota

- Gmail: `labels.create` 5 units once. Mirroring adds one `modify` (5) per
  new inbound message whose tab differs, batched (`batchModify`, 50 for up
  to 1000) when more than ten share a change. Charged to the shared Google
  budget like every push.
- Zoho: one `updatemessage` per mode per run (100 messages per call).

## Risks

- **Mistaken sorts become visible in Gmail.** Accepted; see decision 7.
- **The mark name collides with a member's own label.** A label the member
  already named `Campbooks/Receipts` is adopted as the mark. That is
  harmless: it means the same thing.
- **Mirror volume during a backfill.** The decide pass runs on backfilled
  mail too, which would mirror the whole window. That breaks the "no
  relabelling of old mail" non-goal. Guard: mirror only messages received
  after the inbox's first sync completed (the connector already knows when
  the backfill ends; carry or record that on the inbox). Backfilled mail is
  left as the provider has it.

## Migration slots

`20261023080000_connectors__zoho_labels.sql`,
`20261023080100_conversations__message_provider_stream.sql`. Check the open
PRs' migrations before pushing; if a merge coordinator session is active,
ask it for a slot.
