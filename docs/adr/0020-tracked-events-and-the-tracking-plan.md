# 0020 — Tracked events: a tracking plan in a workbook, a transactional outbox, RabbitMQ streams

**Status:** Accepted · 2026-10-23 · amends [0011](0011-scaling-shape.md) (broker, for egress only) · campbooks-cloud#319

## Context
We want a record of every action in the system: who did it (a person, an
API client, Scout on a person's behalf, an automation, a system job, an
instance admin, or nobody yet signed in), in what context, and what the
records it touched looked like at that moment. Features will be built on
this later: a usage warehouse, an admin view, audit trails, triggers,
models. Each of those must be able to trust that the record is complete and
that it means the same thing everywhere.

Domain events (ADR 0012) don't fit this job. They are contracts between
modules: few, shaped for their consumer, and published only where another
module has to react. They carry no actor and no snapshot, and widening them
would make analytics needs part of the module contracts.

## Decision

**A separate layer of tracked events, defined by a tracking plan.**

- **The plan is a workbook**, `tracking/tracking-plan.xlsx`, and it is the
  source of truth. Sheets: *README*, *Events*, *Properties*, *Records*
  (snapshot shapes shared across events), *Enums*, *Actors*, *Coverage*
  (each operation, job, schedule and event handler → the events it emits, or
  an exemption with its reason) and *Changelog*. Columns that take a closed
  set of values carry dropdown validation.
- **`cargo xtask tracking`** reads the workbook and writes two things. The
  typed Rust events go to a generated file in each owning crate. A canonical
  text mirror, `tracking/tracking-plan.json`, holds every event's JSON
  Schema, so PR diffs can be read and subscribers can validate. `--check`
  fails when either output is stale or the plan breaks a rule. `fmt`
  rewrites the workbook in its canonical form. `apply <patch.yaml>` adds or
  updates rows, so agents and scripts never edit the binary by hand. Nobody
  edits the mirror.
- **Names** are `<module>.<object>_<past_verb>` (`conversations.conversation_archived`)
  with an integer version. A published version never changes meaning; a
  change that breaks a field is a new version.
- **`track(&mut tx, &event)`** writes the event to an outbox,
  `kernel.tracked_events`, **in the transaction of the change**. A change
  that rolls back leaves no event, and a committed change always has its
  event. Only types generated from the plan can be tracked.
- **The envelope is added by the kernel, not by the caller.** It holds: the
  event id (UUIDv7), name, version, `occurred_at`, workspace, actor
  `{type, id}` with an optional `on_behalf_of`, `operationId`, correlation id
  (the request's trace id, carried into every job and delivery it causes),
  causation id (the job or event that caused this one), server role, app
  version and plan version. An HTTP layer sets the ambient context from the
  principal and the matched operation. The worker restores it for each job
  from the origin stored on the job row. A job acting on its own is actor
  `system` (its kind). A job acting for a request keeps that request's actor
  as `on_behalf_of`.
- **No personal data, anywhere in an event.** Property types form a closed
  list: ids, record references, enums, booleans, integers, decimals,
  durations, timestamps, dates, sizes, currency/country/locale codes, and
  `code` strings with a declared pattern. There is no free-text type. Every
  property is classified (`id`, `pseudonymous`, `operational`), and the plan
  check rejects `personal` and `content`. Names, addresses, subjects and
  bodies never appear. Actor and workspace ids are pseudonymous. The
  warehouse hashes them again with its own key.
- **Coverage is checked.** Every mutating operation (`x-campbooks.mutates`, in
  `openapi.json` and `admin.json`) and every registered job, schedule and
  event handler must appear in *Coverage*, mapped to events or exempted with
  a reason. A `live` event must be tracked somewhere outside tests. Coverage
  is required in CI with no baseline (since campbooks-cloud#332, once every
  module was instrumented).

**Egress to RabbitMQ, with streams.** A relay in the worker role publishes
the outbox, in order, to a topic exchange `campbooks.events`. The routing
key is `<name>.v<version>`. The exchange is bound to a stream of the same
name, kept for 30 days. A row is marked sent only after the broker confirms
it, and while the broker is unreachable rows wait in Postgres with backoff.
Delivery is at least once, and subscribers deduplicate on the event id.
Readers that need history replay the stream from an offset or a time.
Readers that need work-queue behaviour bind their own quorum queues with a
dead-letter queue. `CAMPBOOKS_TRACKING__SINK` is `none` (the default: the
outbox isn't written and nothing else runs) or `amqp`.

**Subscribers, not features in the emitter.** The warehouse loader
(not-a-camp-analytics) keeps every event permanently, pseudonymised. The
admin module keeps 90 days for the admin event view: filters, detail, live
tail, and a trace by correlation id. Both read the stream like any future
consumer would.

**Publishing events is part of done.** AGENTS.md, CONTRIBUTING, the PR
template, the OpenSpec `design` and `tasks` rules and the scaffold
templates all ask for it, and the plan check enforces what can be enforced.

## Consequences
- The plan is a binary file, but every change to it appears as a readable
  diff of the mirror, and CI fails when the two disagree.
- One extra insert per tracked action. With the sink off it costs nothing.
- The hosted cloud runs RabbitMQ. Self-hosters run nothing new unless they
  choose to.
- The warehouse holds history beyond the stream's 30 days. Replaying older
  history goes through the warehouse, not the stream.
- Events hold no personal data, so erasing an account or workspace deletes
  nothing downstream. Its ids stop pointing anywhere once the source rows are
  gone.

## Alternatives considered
- **Extending domain events with an actor and snapshots, and relaying
  `kernel.events`**: one system, but analytics would shape the module
  contracts, and many actions (sign-ins, reads, AI runs, admin work) are not
  domain events.
- **Tracking from middleware only** (one generic event per request): no
  developer effort, but no record snapshots, no system actions, and nothing
  meaningful beyond "this endpoint was called".
- **A YAML/JSON plan**: easier to diff, but the owner asked for a proper
  tracking-plan workbook. The generated mirror gives the diff anyway.
- **NATS JetStream**: lighter, with replay. RabbitMQ was preferred because it
  was asked for and is widely known, and its streams give the same replay
  next to classic work queues.
- **Kafka / Redpanda**: the highest throughput and the best fit for replay,
  but too heavy for the shared host at a volume far below what either option
  handles.
- **Writing to the broker directly from handlers**: an event could be lost
  between commit and publish, or announced for a change that rolled back.
