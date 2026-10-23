# Proposal

## Why

campbooks-cloud#320, the first step of epic #319. We want a complete,
trustworthy record of every action: who did it, in what context, and what
the records it touched looked like at the time, so the features built on it
later (a usage warehouse, an admin event view, audit trails, triggers) all
read one record. Domain events are contracts between modules and are not
that record (ADR 0020).

This change lays the foundation. A tracking plan kept as an Excel workbook
in the repo; typed events generated from it; `track()` writing an outbox in
the transaction of the change, with the actor and context added by the
kernel; `cargo xtask tracking` keeping the code, the plan and its readable
mirror in agreement; and the docs that make tracking part of every feature's
definition of done. One existing flow is tracked end to end as the
reference. Publishing to RabbitMQ is the next change (#321).

Modules touched: the kernel (new `tracking` module; job origin), `server`
(the HTTP layer that sets the context, the composition, the tracking config),
a new tooling crate `crates/tracking-plan` (the workbook's tooling, which
`cargo xtask tracking` wraps), `conversations` (the reference flow:
archiving a conversation). New dependencies: `calamine` (read the workbook),
`rust_xlsxwriter` (write it) and `serde-saphyr` (YAML patches), in the
tooling crate only. Three kernel migrations. No new endpoint, scope or
secret.

## What Changes

- `tracking/tracking-plan.xlsx`: the plan, with sheets README, Events,
  Properties, Records, Enums, Actors, Coverage and Changelog, plus the
  envelope and actor types it describes.
- `tracking/tracking-plan.json`: the generated, canonical mirror with a
  JSON Schema per event.
- `crates/tracking-plan` (`cargo run -p campbooks-tracking-plan -- check |
  generate | fmt | apply <patch.yaml> | init`), wrapped by
  `cargo xtask tracking [--check | generate | fmt | apply <patch.yaml>]`.
- Kernel `tracking`: the `TrackedEvent` trait (implemented only by generated
  code), `Tracker::track(&mut tx, &event)`, the ambient `TrackingContext`
  (actor, on behalf of, operation, correlation, causation, role), the
  `kernel.tracked_events` outbox with row-level security, and
  `CAMPBOOKS_TRACKING__SINK` (`none` by default; `outbox` writes the outbox
  without publishing, for tests and until the relay lands; `amqp` comes with
  #321).
- `kernel.jobs.origin`: the actor and correlation a job was enqueued under,
  ids only, so work a request causes is attributed to that request.
- Reference: `conversations.archive` tracks
  `conversations.conversation_archived` with a conversation snapshot.
- ADR 0020; AGENTS.md "Tracked events (enforced)"; `docs/tracking.md`;
  CONTRIBUTING; the PR template; OpenSpec config rules; scaffold templates.
- CI: `cargo run -p campbooks-tracking-plan -- check` in the Rust job, and a
  server test checking Coverage against the real registry.

## Non-goals

- Publishing to a broker (#321), broker infrastructure (#322), subscribers
  (#323, #324).
- Instrumenting every module (#325–#331). Coverage is checked for the
  entries that exist; requiring full coverage is #332.
- Tracking failed requests: telemetry covers those. A failure that matters
  to the product (a send that failed) is a plan event of its own.
- Any personal data in events.
