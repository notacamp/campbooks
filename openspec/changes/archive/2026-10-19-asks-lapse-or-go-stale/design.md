# Design

## Data ownership
- `conversations.need_summaries` (one row per message, written by `conversations.summarize_need`) gains `lapses_at timestamptz`, `due_on date` and `dates_read_at timestamptz`: what Scout read about the message's dates, and when it was asked about them (NULL for rows written before this change). The dates belong to the message, like the summary: a second need on the same message must not ask again (the reason the summary has its own table).
- `conversations.needs` gains `lapses_at timestamptz` (it already has `due_on`): the open need's copy, filled by the refresh from the message's summary row, so `raise` stays a function of the answers and the stored row compares as before. A need that cannot lapse (Pay, a phishing warning) carries none.
- `conversations.needs` widens its checks: question `still_to_do` and `still_needs_reply`, cleared cause `lapsed`. Widened `NOT VALID` and validated in a migration of its own, as before.
- `conversations.need_clocks` (new): `conversation_id` (primary key, references `needs`, cascading), `workspace_id`, `wake_at`. One row per open need that will change on its own, and when. Forced row-level security per workspace, plus `maintenance_reads_need_clocks` (`FOR SELECT`, maintenance scope): ids and times only, read only.
- `attention.items` gains `lapses_at timestamptz`.

## Events
`conversations.need_changed` v1 gains `need.lapses_at` (optional RFC 3339 time; `#[serde(default)]` on both sides so old events replay and old consumers ignore it). A time is not personal data. The attention module stores it on the item. Clearing as `lapsed` uses the existing `cleared_because`. The contract (`contracts/conversations.need_changed.v1.schema.json`) and `openapi/events.json` say so.

## AI use
Role `complete`, the same single call that writes the summary (no new call). The instruction gains: return `lapses_at` and `due_on` from what the email says and `sent_at`, null when it says nothing; never write time relative to now. The answer is still held to a strict JSON schema (both fields `string | null`, required). Rust validates: `lapses_at` must parse as RFC 3339 and `due_on` as `YYYY-MM-DD`; anything before the message was sent, or more than 366 days after it, is dropped. The answer's dates are kept even when its texts are not usable, since they stand on their own.

Fallback: without a `complete` role that may read mail here, no dates are read. Needs then never lapse, and old asks still become questions (that rule needs no model). Undo: a lapse is not a member's data; the member can still reply, archive or act on the conversation, and bringing an expired ask back by hand is an open decision (`decisions-needed.md`). A still-needed question is undone with the existing `reopen_need`.

## Settling with a clock
`settle(need, since, clock)` stays pure. `Since` gains the message's `sent_at`; `Clock` is `now` and the needs window. Order: member verdicts, a taught rule, the trash, a reply, an archive (as before), then:

1. **Lapsed**: the need has a `lapses_at` at or before now and is not Pay or a warning → cleared `lapsed`, whatever its basis (a confirmed ask still cannot be done once its link is dead).
2. **Still needed?**: a `sure` `do` or `reply` need, basis not `member`, no due date, no lapse time, whose message was sent longer ago than the window → reason `review`, trust `check`, question `still_to_do` or `still_needs_reply`.

A member's Yes (`confirm_need`) maps the question back (`still_to_do` → `do`, `still_needs_reply` → `reply`) with basis `member`, so rule 2 never applies to it again. Not needed clears it `not_needed`. Neither stores a correction: the decide answers were right, and a label saying otherwise would teach the evals the wrong thing. For the same reason the client offers no "teach once" after it and its toast does not say Campbooks will learn.

A need with a due date is left to the due date (overdue, then attention's 7-day cutoff); one with a lapse time ahead is left to it.

## The clock-driven refresh
After every refresh the conversation's clock row is written: for an open need, the earliest of its `lapses_at` (when it can lapse) and its message's `sent_at` + window (when rule 2 could apply), if in the future; otherwise the row is deleted. It is written in the same transaction as the need, after the needs row is locked, in the same order every caller of `refresh` already takes (needs row, then this).

- `conversations.dispatch_need_clocks` (schedule, every minute, queue `maintenance`) reads in the maintenance scope which workspaces have a clock row due (`SELECT DISTINCT workspace_id … WHERE wake_at <= now()`, at most 500) and enqueues `conversations.wake_needs` in each, one pending job per workspace. It reads nothing else and writes nothing.
- `conversations.wake_needs` (queue `rules`, unit payload) reads up to 200 due conversation ids in its workspace, then works each out in a transaction of its own (`begin_for`): `needs::refresh`, then deletes the clock row if it is still due (the refresh moved it or removed it otherwise). One transaction per conversation means the job never holds one conversation's locks while waiting for another's. A conversation that fails is logged by id and pushed an hour ahead, so it cannot spin. The next tick takes what is left.

`attention.list` also hides an open item whose `lapses_at` has passed, at read time, so a ten-minute link leaves Home at once.

### Alternatives weighed
- **A maintenance read policy on `conversations.needs`** (a `wake_at` column there): one table less, but the dispatcher's scope would then reach every column of every workspace's needs (reason, question, verdicts, member ids). A narrow table holding ids and times keeps the cross-workspace surface to what the dispatcher needs, as `connectors.sync_due` does.
- **A delayed job per need** (`run_at` at the wake time): no table and no maintenance scope, but a job that fails its attempts is gone and the need never changes; pending jobs pile up for two weeks per ask; a moved wake time leaves the old job behind. The clock row is the durable record of what is still to happen, and the dispatcher picks it up whatever happened before.
- **Read time only** (attention and the reader each work out "lapsed" and "stale" when they read): no background work, but the conversations need, its event and the attention item would disagree, and a still-needed question has to exist as a need to be confirmed, undone and counted in Review. Kept for the lapse on Home only, as a complement.
- **Asking the `decide` model**: its questions are fixed and typed (booleans, levels, labels, ADR 0018) and the decision protocol has no date answer; the summary call already reads the email when the need opens.

## The question's shape
New `NeedQuestion` values `still_to_do` and `still_needs_reply` rather than a flag: the question is what the member is asked, the client words it from the question alone, the confirm mapping stays a match on the question, and the check constraint names every kind. The reason becomes `review` so Home shows it under Review with Yes and No, as every other question.

## Expired in the inbox
`ConversationSummary.lapsed_need` (reason, question, message id, lapse time) is set while the conversation's need lapsed (cleared `lapsed`, or open with its lapse time passed and not yet woken) and that message is still the conversation's newest inbound one. It is a separate field so every reader of `need` keeps meaning "an open need". The list and the reader show an "Expired" chip from it.

## Bills and the 7-day cutoff
Bills now get due dates, and `attention.list` dropped every item 7 days after its due date. A Pay item is kept past it: a bill stays until a member answers it.

## Upgrade
The migration queues every inbox for the needs backfill. Its page job refreshes each conversation with an open need (settling it against the clock and writing its clock row), and queues `conversations.summarize_need` for each open need whose summary row has no `dates_read_at`, besides those with none. The job then asks once more for a message already summarised: on a usable answer it replaces the texts and stores the dates; on no usable answer it keeps the old texts and marks the row read, so it is never asked again. The call counts against the workspace's daily AI limit, like every summary.

## Endpoints and scopes
No new endpoint. `conversations.list` and `conversations.get` (`conversations:read`) return the new fields; `conversations.confirm_need` and `conversations.resolve_need` (`conversations:write`) accept the new questions; `attention.list` (`attention:read`) filters as above.

## Known limits
- Done after a Yes on a still-needed question, then Undo, asks again (a Yes stores no correction to restore from).
- A `lapses_at` the model got wrong clears an ask; the member still has the conversation, and the Expired chip says why it left Home.
