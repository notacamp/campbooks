# Design

## Context

`conversations` publishes `conversations.changes_requested` v1 when a member
(or automation, or the assistant) changes a conversation: one item per
affected message, each with a change kind (`mark_read`, `mark_unread`,
`star`, `unstar`, `archive`, `unarchive`, `add_label`/`remove_label` with an
optional provider key, `move_to_stream`). It never publishes it for changes
that came from the provider. `connectors` owns the Gmail stream that feeds
the inbox, its cursor (Gmail history id), an index of every message it
emitted (`connectors.gmail_messages`, with Gmail's label ids), the shared
per-provider quota budget, and the run that reads one page of a stream
(`connectors.sync_stream`), which is also where backoff, throttling and
health live.

## Data ownership and events

- **Schema `connectors`** (migration `connectors__outbound_changes`):
  - `outbound_changes`: one row per message per intent: stream,
    `request_id` (the event id; unique with the message id, so a replayed
    event records nothing twice), Gmail message id, change kind, Gmail label
    ids to add and remove, `stamp_history_id`, `state`
    (`pending` → `applied` | `superseded` | `skipped` | `failed`), `reason`
    (a stable code), `attempts`, the history window of the write
    (`applied_from`, `applied_to`), timestamps. No subjects, addresses,
    label names or bodies. Rows that are done are pruned after
    `CAMPBOOKS_CONNECTORS__BATCH_RETENTION_DAYS` (7); a row still pending
    after that long is `failed` (`expired`).
  - `gmail_messages.reemit`: the message's state must be sent to
    conversations again (its member change was overtaken or refused).
  - Row-level security as on every workspace table.
- **Consumed:** `conversations.changes_requested` v1 by the new consumer
  `connectors.request_changes` (queue `events`), read with the module's own
  type validated against the contract file. Unknown change kinds or streams
  (a later, additive contract version) are recorded as `skipped`
  (`unsupported`) rather than failing the delivery.
- **Published:** nothing new. Write-backs that change what Campbooks
  should show (a superseded or refused change) reach conversations as an
  ordinary upsert in the next `connectors.messages_changed` batch, with
  origin `provider` on their side.
- **Endpoints and scopes:** none. Google scope: the existing `gmail.modify`.

## The flow

1. **Record** (`connectors.request_changes`, per event). Find the stream
   that feeds the event's inbox (none: the inbox is not a connector's, or
   it was disconnected; nothing to do). Map each item to Gmail labels:

   | Intent | Gmail |
   | --- | --- |
   | `mark_read` / `mark_unread` | remove / add `UNREAD` |
   | `star` / `unstar` | add / remove `STARRED` |
   | `archive` / `unarchive` | remove / add `INBOX` |
   | `add_label` / `remove_label` with a provider key the stream's label cache knows | add / remove that label id |
   | the same without a key, or with a key Gmail no longer has | `skipped`, `label_not_at_provider` |
   | `move_to_stream` primary / updates / newsletters | add `CATEGORY_PERSONAL` / `CATEGORY_UPDATES` / `CATEGORY_PROMOTIONS`, remove the other four categories |
   | `move_to_stream` receipts | `skipped`, `no_provider_equivalent` |

   Only `UNREAD`, `STARRED`, `INBOX`, the five categories and user labels
   (the stream's label cache, `kind = user`) are ever written: a label
   change naming a system label is skipped, and a mapped change that
   would touch any other label is recorded `skipped` (`unsupported`).
   Stamp the rows with Gmail's current history id (`getProfile`, 1 unit,
   from the provider budget); when Gmail cannot be asked (throttled, no
   budget, refused token) the stamp is empty and the change simply applies
   in arrival order. Then, in the delivery's transaction: lock the stream
   row, insert the rows, and ask the stream to run now
   (`schedule::run_now`, unique per stream, so many intents collapse into
   one run). A paused stream (needs re-authorisation) keeps its rows
   pending until it is connected again.
2. **Push** (first step of `connectors.sync_stream`, after the token and
   before the page is read). First, pending rows older than the batch
   retention are given up (`failed`, `expired`) and their messages flagged
   `reemit`: a change is never made late after a long pause or a
   re-authorisation. Then take up to 500 pending rows of the stream in
   recording order and:
   - **skip what would undo the trash**: a row that would put `INBOX` or a
     category on a message the index holds in `TRASH` or `SPAM` ends
     `skipped` (`in_trash`), and the message is re-emitted.
   - **merge** them per (message, label): the latest row wins; earlier rows
     for the same label end `superseded` (`replaced`). Archive then undo
     before the run is one no-op, not two writes.
   - **check for later writers**: read Gmail's history from the oldest stamp
     (`history.list`, 2 units a page, at most four pages). A record that
     adds or removes the same label on the same message after the
     winning row's stamp, and is not inside the history window of one of
     Campbooks' own earlier writes of that label, means someone changed it
     in Gmail later: the write is dropped (`superseded`,
     `changed_at_provider`) and the message is flagged `reemit`. A row is
     **atomic**: when any label it won is overtaken, none of it is written
     (a move between categories must never leave two). A deleted message
     ends `skipped` (`message_gone`). History Gmail no longer has
     (404), or more than four pages of it, skip the check: arrival order.
   - **write**: group messages by identical (add, remove) sets. A group of
     more than ten uses `messages.batchModify` (50 units for up to 1,000
     messages, cheaper than 5 each from eleven on); the rest use
     `messages.modify`, five at a time. A batch Gmail refuses (4xx other
     than 429) is retried message by message, so one bad id cannot sink the
     others. Per message: 404 → `skipped` (`message_gone`); another refusal →
     `failed` (`refused`) and `reemit`.
   - **trouble**: a 429, 5xx, network error or refused token stops the push.
     What was written is recorded; the rest stays pending with
     `attempts + 1` and the run ends through its existing trouble path
     (throttled until `Retry-After`, or exponential backoff from
     `poll.min_seconds`, health `throttled`/`failing`/`needs_reauth`). A row
     that has hit trouble 8 times ends `failed` (`provider_errors`) and is
     re-emitted.
   - **record** in one transaction: row states, the run's history window
     (`applied_from` = Gmail's history id before writing, from the history
     page or `getProfile`; `applied_to` = `getProfile` after writing) on
     every applied row, and the index of each written message advanced to
     `labels ∪ add − remove`. When the `getProfile` after writing fails, the
     window is left open and the run's read closes it at the history id it
     reaches (past our writes, since the read comes after them); a window
     still open after ten minutes covers nothing (our own write may then be
     taken for Gmail's, which only means Gmail's state wins).
   - The run's budget is taken once for pushing and reading together.
3. **Pull** (the existing read). Messages flagged `reemit` are re-checked
   and emitted even when their labels did not change. For a message with
   pending rows (recorded while this run was going), the emitted state has
   those rows applied, so a stale read never undoes a member's change in
   Campbooks. At the end, if rows were recorded during the run, the next
   run is enqueued at once (as when a page says there is more).

## Echo suppression

Three rules, each closing one loop:

1. **Origin.** Changes from Gmail reach conversations as batches; it never
   publishes `changes_requested` for them, so nothing from Gmail is ever
   written back.
2. **The index moves with our write.** After writing, the message's index
   row holds `labels ∪ add − remove` (not Gmail's answer, which could carry
   an unrelated change Campbooks has not read yet). When Gmail's history
   then reports our write, the run's re-check compares Gmail's labels with
   the index, finds them equal, and emits nothing: no batch, no event. A
   Gmail change made meanwhile still differs from the index and is emitted.
3. **Pending overlay.** A run that reads a message while a member's change
   to it is still pending emits it with that change applied, so
   conversations never sees (and never re-sorts on) the state the member
   just changed.

## Conflicts: last writer wins, in Gmail's order

Gmail has no per-message precondition (no etag on `modify`). Its history is
the one total order of writes to a mailbox, so it is the clock:

- A member's change is placed at the Gmail history id stamped when the
  connector recorded it (within a second of the click in normal operation).
- Per message and label, the **later** writer wins. A change at Gmail
  **after** the stamp wins over the member's: their write is dropped and
  Campbooks is told Gmail's state. A change at Gmail **before** the stamp
  (the member did not see it yet) loses: the member's write is made.
- Two member changes: the later recorded wins (merge), and a member's
  change recorded while an earlier one of theirs was being written is not
  mistaken for a Gmail change: records inside the history window of a run
  that wrote the same label are Campbooks' own.
- **Limits, documented:** a change without a stamp applies in arrival order;
  history older than Gmail keeps (about a week) or longer than four pages
  is not scanned (arrival order); a Gmail change inside a run's own write
  window (the milliseconds between `history.list` and the last `modify`) is
  taken for Campbooks' own.

## Alternatives considered

**Where write-backs run.**
(a) A `connectors.push_changes` job per event, sharing the stream's ordering
key so it never interleaves with a sync. Rejected: a job waiting on a
`Retry-After` is future-dated, and an unfinished job with an ordering key
blocks every later job with that key, so one throttled write would stop the
stream's reads; it also needs its own backoff, health and budget code.
(b) A push stream with its own schedule row. Rejected: a second schedule
per connection for the same quota and the same failure modes.
(c) **The sync run pushes first, then pulls (chosen):** one place for the
token, the budget, throttling, backoff and health; pushes and pulls of a
stream never interleave; "run now" already collapses bursts. Cost: a
member's change waits for a running page to finish (seconds).

**Per message, per thread, or batched.**
(a) `threads.modify` (10 units): Gmail threads are not conversations
(conversations threads by headers too), and intents are per message (mark
unread touches only the newest, star only the newest), so it would change
messages the member did not touch. Rejected.
(b) `messages.batchModify` always (50 units): five times the cost of a
single modify for the usual one to five messages, and it answers nothing per
message. (c) Gmail's HTTP batch endpoint: multipart parsing for the same
quota. (d) **`modify` per message, `batchModify` for a group of more than
ten identical changes (chosen):** cheapest in units at every size, and
per-message answers where groups are small.

**How echo suppression works.**
(a) Rely on conversations ignoring no-op upserts (the earlier sketch):
every write still comes back as a batch and an event, and an interleaved
read can briefly undo the member's change. (b) Tag our writes and skip
their history records in the reader: Gmail does not say which history id a
`modify` produced, so it needs the same window as the conflict check plus
reader changes, and still needs (a)'s fix for stale reads. (c) **Index
advanced with the write, plus the pending overlay (chosen):** the reader's
existing comparison does the work, nothing is emitted for our own writes,
and Gmail changes still show.

**The conflict rule.**
(a) Provider wins on any difference (a compare-and-set against what the
member saw): simple, but it drops a member's later change whenever Gmail
changed the message earlier, which is not last writer wins. (b) Arrival
order at Gmail: no extra calls, but a write delayed by throttling would
overwrite a newer change made in Gmail. (c) **Last writer wins in Gmail's
history order, with a stamp at recording time (chosen):** 1 unit per intent
and 2 per run, and the order is Gmail's own, not two clocks compared.

**Labels made in Campbooks.** Creating them in Gmail needs their names, and
the intent carries none (names are personal data and travel in blobs).
Skipped and recorded; the options are in `openspec/decisions-needed.md`.

**Copied from the Rails app on `main`?** No. The Rails app called the
provider inside the request (mark read updated the local row, then the
provider, then the live inbox). Here the request never waits on Gmail, the
write is durable before it is attempted, and retries, quota and conflicts
are the connector's.

## Risks / Trade-offs

- **Quota.** Each recorded intent costs one `getProfile`; a burst of
  intents is collapsed by the run, not by the recording. Bounded by the
  provider budget.
- **Latency.** A change reaches Gmail on the next run, normally within a
  second; behind a long backfill page, after it.
- **Unverified against real Gmail.** `batchModify`'s answer for an id it
  does not know is not documented; the client retries a refused batch one
  message at a time whatever the cause. The staging check with a real test
  account (#53) covers it.
