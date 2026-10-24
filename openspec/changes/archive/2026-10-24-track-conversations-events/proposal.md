# Track the conversations module's changes

Module: `conversations` (its operations, jobs and event handlers), with the
tracking plan (`tracking/`). Tracking:
https://github.com/notacamp/campbooks-cloud/issues/325, part of epic
https://github.com/notacamp/campbooks-cloud/issues/319.

## Why

ADR 0020 asks for a record of every action: who did it, in what context,
and what the records it touched looked like then. Only the reference flow
(archiving a conversation and putting it back) is tracked so far. The
conversations module holds most of the product's actions (reading, sorting,
labelling and binning mail, drafts and sends, needs, taught rules, Scout's
reads), so the warehouse and the admin event view see almost nothing until
it is instrumented. Full coverage becomes a CI failure with
campbooks-cloud#332, so every operation, job, schedule and handler of the
module needs a Coverage row first.

## What Changes

- Tracking plan version 2 (`tracking/patches/0002-conversations.yaml`): 40
  new live events of `conversations`, the snapshot records
  `conversations.draft`, `conversations.need`, `conversations.rule` and
  `conversations.rule_run` next to the existing `conversations.conversation`,
  their enums, and a Coverage row for each of the module's 38 operations
  (37 mutating, and the read-only rule preview), 15 jobs, 3 schedules and 9
  event handlers.
- Every live event is tracked with `Tracker::track` in the transaction of
  its change, after the change, from the module's own rows read in that
  transaction; a no-op tracks nothing.
- Sends are tracked by their ids, kinds, states and counts only: scheduled,
  taken back, handed to the connector, sent, or failed with an error kind.
- A test per event reads it back from `kernel.tracked_events`.

No new endpoint, scope, migration, dependency or domain event. No change to
what any operation returns.

## Non-goals

- The other modules (#326–#331) and the hard coverage gate (#332).
- Mailbox, provider and sync work owned by `connectors` (#329): this change
  tracks only what the conversations crate registers.
- Tracking reads, or Scout's proposed reply text (nothing is stored; a
  draft made from it is tracked as a draft).
- Any personal data or content in events: no names, addresses, subjects,
  bodies, snippets, label names, note text, rule conditions or filenames.
