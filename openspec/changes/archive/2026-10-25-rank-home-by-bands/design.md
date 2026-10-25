# Design

## Context

`attention.list` returns the caller's open items ordered by reason, then
overdue, due date, urgent and age, and Home draws one group per reason.
`urgent` is Scout's own reading of the email, whose precision is below the
floor (campbooks-cloud#361), and the reason says what kind of thing an item
is, not how pressing it is. The approved prototype ("Priority 2 · Bands",
"Rows B · Why first") orders by band, and puts the reason first on the row.

## The bands

`attention::domain::bands` is pure: `band(item, today, now)` returns the
band and why from an item's facts, first rule wins.

| # | When | Band | Rule (data) |
|---|---|---|---|
| 1 | the suspicious-mail warning | now | `warning` |
| 2 | due before today | now | `overdue` (`days` late) |
| 3 | due today / tomorrow | today | `due_today` / `due_tomorrow` |
| 4 | a task planned (`date`) for today or a day before | today | `planned_today` |
| 5 | a nudge | today | `nudge_due` (`on`: when an answer was expected) |
| 6 | a reply a person has waited 3 whole days or more for | today | `waited` (`days`) |
| 7 | due within the next 7 days | week | `due_on` (`on`) |
| 8 | a reply or a choice a person is waiting for | week | `waiting_answer` |
| 9 | "Worth a look" | whenever | `kept_by_rule` |
| 10 | otherwise | whenever | `due_on` (`on`) when dated, else `no_date` |

Two refinements of the brief's table, both judgment calls:

- A nudge's `due_on` is when Scout expected an answer, so it is always in
  the past once the wait turns into a nudge. Rules 2, 3 and 7 skip nudges;
  rule 5 carries the expected day so Home can say "Rui's answer was due
  Fri".
- An item dated more than a week ahead is Whenever with `due_on`, not
  `no_date`: Home must not say "No date" over a bill due on the 30th.

`today` is the caller's day (`utc_offset`, as for overdue). Inside a band
the order is one key (`sort_key`): band, warnings first, dated before
undated, the effective date (due date, else planned day) ascending, which
also puts the longest overdue first, people before services and the unknown,
`since` ascending, then the item id, so two reads never differ.

## Data ownership

- **attention** owns the bands. Migration
  `20261107020000_attention__bands.sql` adds to `attention.items`:
  `warning boolean NOT NULL DEFAULT false`, `from_person boolean NULL`,
  `plan text NULL`, `plan_on date NULL` (catalog-only). The list reads them
  with a runtime query (no `.sqlx` entry), computes the band per row and
  sorts in Rust. `ItemView` gains `band` and `band_reason`, set on the open
  list only: `null` on `state=cleared`, `state=waiting` and the snooze
  response, where a band means nothing (a wait's date is when Scout checks,
  Done today is history). `AttentionList` gains `tasks_elsewhere`.
- **conversations** owns `warning` and `from_person`. Neither is stored: both
  are worked out when the need is announced (`refresh_pass`). `warning` is
  the need's question being `suspicious` (the client's test until now);
  `from_person` is `!facts::is_machine_sender` over the message's envelope
  (headers only, ADR 0018), `None` when the message cannot be read.
  Migration `20261107020100_conversations__need_facts.sql` adds
  `conversations.needs.facts_version smallint NOT NULL DEFAULT 0`.
- **tasks** owns the plan: `tasks.task_changed` already carries `plan_on`;
  attention's view of the contract reads it now.

## Events

- `conversations.need_changed` v1: the `need` object gains two optional
  fields, `warning` (absent when false) and `from_person` (absent when
  unknown). Additive: the contract
  (`contracts/conversations.need_changed.v1.schema.json`, `additionalProperties:
  false`) and `openapi/events.json` list them; old events still parse
  (attention reads them with `#[serde(default)]`), and no version bump.
  Neither field holds personal data: two booleans.
- `tasks.task_changed` v1: unchanged; attention now reads `plan_on`.

## Backfill

The facts must reach existing open items right after deploy, or every reply
from a person would fall to Whenever.

- **Needs.** A refresh announces a need only when it changed. The new
  `facts_version` says which facts an open need was last announced with
  (`NEED_FACTS = 1` now); `worth_announcing` also says yes for an open need
  announced with older facts, and every write stores the current version.
  The migration's seed gives every open need a clock due now, so the minute
  dispatcher has `conversations.wake_needs` work each one out again: the
  owning module re-publishes, no cross-schema SQL, no model call and no rule
  run (`refresh` reads stored answers only). Running the seed again changes
  nothing. The needs backfill (`needs_backfills`) was not reused: it also
  queues the model pass for unanswered mail.
- **Tasks.** The tasks module has no re-publish path, and building one (a
  table, a dispatcher, Coverage rows) is out of scope. The smallest correct
  thing: a task item projected before this change has `plan` NULL, and the
  list keeps showing it as it did (on Home, banded by its due date) until
  its next `task_changed` stores the plan. Nothing that was on Home
  disappears at deploy; a task planned for a day shows as planned once it
  changes. `warning` defaults to false and is corrected by the needs
  re-announcement within minutes.

## Only today's tasks

A task item is listed when it is overdue, due today or tomorrow, planned
(`date`) for today or a day before, or back from Later (the list only sees a
Later task once `hidden_until` has passed, and the attention spec says it
comes back by itself). Others are counted in `tasks_elsewhere`. A
conversation's Do or Pay item replaced by a task stays replaced (unchanged
SQL), so a task due Friday moves the lease email off Home into "1 more in
Tasks".

## Scout

`what_needs_me` already reads `attention.list` in its order; it now passes
`band` and `band_reason` instead of `urgent`, and `tasks_elsewhere`. The
tool description and the assistant handbook line say "most pressing first".

## Client

- `model.ts`: `groupByBand` replaces `groupByReason` (server order kept
  inside a band); `bandOf` falls back to `whenever` for an item without one.
- `lib/ui` `Item` gains an `eyebrow` (`ItemEyebrow`: signal, kind, reason,
  danger): one line before the name, `xs` semibold capitals,
  `--tracking-caps`, tertiary, the dot, the kind, `·`, then the reason in
  secondary or danger. A long reason wraps rather than being cut, so the
  copy-fit check reads it whole. `QueueGroup` takes no signal (no dot) and a
  `danger` tone for Now.
- Reason sentences come from `band_reason` in the member's language; names
  are the first name Home already derives (`firstNameOf`), with a neutral
  sentence when there is none.
- "Why it's here" in the opened item: the visor, "In Today because Marta has
  waited 4 days." and the band's description.

## Scopes

Unchanged: `attention.list` still requires `attention:read`.

## AI

None. No model is called; every band comes from structured facts (dates,
reasons, headers). The member undoes nothing because nothing is changed:
the bands only order.

## Tracking coverage

Checked `tracking/tracking-plan.xlsx` (Coverage) and `docs/tracking.md`:
no operation, job, schedule or handler is added, and no tracked action
changes (opening "Why it's here" is not an action). The handlers whose
behaviour grows (`attention.project_conversation_need`,
`attention.project_task`, `conversations.wake_needs`) keep their rows. The
`attention.item` snapshot is unchanged. Tracking plan unchanged.
