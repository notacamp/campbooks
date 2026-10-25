# Track the assistant, attention, search and system modules' changes

Modules: `assistant` (Scout's chats, proposals and each member's profile),
`attention` (each member's "Later"), `search` (the index) and `system` (the
admin job actions and the AI daily limit), with the kernel's job queue that
the admin job actions change, and the tracking plan (`tracking/`).
Tracking: https://github.com/notacamp/campbooks-cloud/issues/330, part of
epic https://github.com/notacamp/campbooks-cloud/issues/319.

## Why

ADR 0020 asks for a record of every action: who did it, in what context,
and what the records it touched looked like then. These four small modules
are not tracked yet: a member starts, continues and deletes chats with
Scout and confirms or declines what it proposes, changes how Scout acts,
puts items off until later; an administrator retries and discards dead
jobs; a workspace runs out of its daily AI calls. Full coverage becomes a
CI failure with campbooks-cloud#332, so every mutating operation, job,
schedule and handler of the four modules needs a Coverage row first.

## What Changes

- Tracking plan version 7 (`tracking/patches/0007-assistant-attention-search-system.yaml`):
  15 new live events (8 of `assistant`, 2 of `attention`, 1 of `system`, 4
  of `kernel`), the snapshot records `assistant.chat`, `.answer`,
  `.proposal`, `.profile`, `attention.item` and `kernel.job`, their enums,
  and a Coverage row for each mutating operation, job, schedule and handler
  of the four modules (33 rows).
- Every live event is tracked with `Tracker::track` in the transaction of
  its change, after the change, from the owning module's own rows read in
  that transaction; a no-op tracks nothing.
- The admin job actions change `kernel.jobs`, which only the kernel writes:
  their events are the kernel's (`kernel.job_retried` and so on), tracked in
  the kernel's transaction. `campbooks_kernel::jobs::admin::{retry,
  retry_kind, discard}` take the tracker.
- The AI usage meter tracks the first refused call of a workspace's day
  (`system.ai_limit_reached`): it is composed with the server's tracker
  (`Composition::from_env` and `ai_gateway` take it).
- A streamed answer is written after the handler returns: its work now runs
  in the request's tracking context, so its events (and the meter's) name
  the member, not the process.
- A test per event reads it back from `kernel.tracked_events`.

No new endpoint, scope, migration, dependency or domain event. No change to
what any operation returns.

## Non-goals

- The other modules (#325–#329, #331) and the hard coverage gate (#332).
- Search queries and every other read: a read changes nothing.
- The operator's `campbooks-server search reindex` command: a CLI step
  outside any operation, job or handler, rebuilding a projection (no
  operation lets a member start a reindex).
- Each AI call: the usage ledger (`system.ai_usage`) is bookkeeping read by
  `system.admin_ai_usage`; only the day's first refusal, which someone
  notices (Scout stops reading new mail), is an event.
- Any personal data or content in events: no question, answer, prompt,
  title, sentence, label, reply intent or any other model output; no chat's
  first question; no screen route or client report; no job payload or error
  text.
