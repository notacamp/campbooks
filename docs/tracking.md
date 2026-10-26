# Tracked events

Tracked events are the record of every action in Campbooks: who did it, in
what context, and what the records it touched looked like at that moment. The
decision and its alternatives are in
[ADR 0020](adr/0020-tracked-events-and-the-tracking-plan.md); the rules are in
[`AGENTS.md`](../AGENTS.md) → *Tracked events*. This page is the how-to.

Tracked events are not domain events. A domain event (ADR 0012) is a contract
between modules, published only where another module has to react. A tracked
event is the product's record of what happened, for the usage warehouse, the
admin event view, audit trails and whatever is built on them later.

## The pieces

| Piece | Where | What |
| --- | --- | --- |
| The plan | `tracking/tracking-plan.xlsx` | The source of truth: every event, its properties, the snapshot shapes, enums, actor types, coverage and changelog. |
| The mirror | `tracking/tracking-plan.json` | Generated. The whole plan as text, with a JSON Schema per event: PR diffs read it, subscribers validate with it. |
| Generated code | `crates/modules/<m>/src/tracking.gen.rs`, `crates/kernel/src/tracking/tracking.gen.rs` | Generated. One struct per event version, one `<Record>Snapshot` per record, one enum per enum. |
| Patches | `tracking/patches/*.yaml` | How agents and scripts change the plan, kept as the history of those edits. |
| The tool | `crates/tracking-plan` | Reads, checks, formats, patches and generates. Depends on no other Campbooks crate, so it runs without building the server. |
| The kernel | `campbooks_kernel::tracking` | `Tracker::track`, the ambient context, the outbox `kernel.tracked_events`, and the relay that publishes it to RabbitMQ. |

Never edit the mirror or a `tracking.gen.rs` by hand: CI fails when they are
not exactly what the workbook generates.

## The plan

Open `tracking/tracking-plan.xlsx` in Excel or LibreOffice. Its README sheet
repeats the essentials. Columns that take a closed set of values have
dropdowns.

| Sheet | One row per |
| --- | --- |
| Events | event version: name, version, module, object, action, scope (`workspace`/`instance`), status (`planned`/`live`/`deprecated`), actor types, description, when it fires, the plan version that added it, the OpenSpec change |
| Properties | property of an event version: type, list, enum or record, required, classification, pattern, description, example |
| Records | field of a snapshot shape (`<module>.<record>`), shared by the module's events |
| Enums | value of an enum (`<module>.<enum>`) |
| Actors | actor type (fixed: `user`, `client`, `assistant`, `automation`, `system`, `admin`, `anonymous`) |
| Coverage | operation, job, schedule or event handler: the events it tracks, or an exemption with its reason |
| Changelog | change of the plan: date, plan version, change, PR |

### Names and versions

- Events are `<module>.<object>_<past verb>`: `conversations.conversation_archived`.
  The Module, Object and Action columns repeat the parts.
- Versions start at 1. A published version never changes meaning: a change
  that breaks a field is a new version, and only the newest version may be
  `live` (older ones are `deprecated`).
- Properties, fields and enum values are `snake_case`. Records and enums
  belong to the event's module (snapshots come from the module's own rows).
- The platform modules `kernel`, `server` and `bff` (the web
  backend-for-frontend, `crates/server/src/bff`) are no crate under
  `crates/modules`: their types are generated into the kernel's
  `tracking.gen.rs`, and `server` and `bff` events are tracked from
  `crates/server/`.
- Each change of the plan adds a Changelog row with the next plan version.
  The plan version is written into every event's envelope.

### Types and classifications

Types: `id`, `record_ref`, `record` (a snapshot from Records), `enum`,
`bool`, `int`, `decimal`, `duration_ms`, `timestamp`, `date`, `bytes`,
`currency`, `country`, `locale`, and `code` (a string from the code, which
must declare its `^…$` pattern). List = yes makes a list of any type but
`record`. **There is no free-text type.**

| Classification | Means | Allowed |
| --- | --- | --- |
| `id` | a record's own id | yes |
| `pseudonymous` | points at a person or workspace without naming them (an account id) | yes |
| `operational` | counts, flags, states, codes, times | yes |
| `personal` | names, addresses, phone numbers | rejected |
| `content` | subjects, bodies, file names, anything someone wrote | rejected |

Write a field down as `personal` or `content` when you considered it: the
check fails until it is removed, so the rejection shows up in review instead
of being dropped silently.

## Adding an event

1. **Plan it.** Write a patch, `tracking/patches/<nnnn>-<slug>.yaml` (the
   next number), with the rows to add or replace (rows are matched by their
   key), or edit the workbook directly:

   ```yaml
   changelog:
     - { date: "2026-11-02", plan_version: 2, change: "Add conversations.conversation_starred", pr: "#700" }
   events:
     - name: conversations.conversation_starred
       version: 1
       module: conversations
       object: conversation
       action: starred
       scope: workspace
       status: planned
       actor_types: [user, client, assistant, automation]
       description: A conversation was starred.
       fires_when: Starring changes its newest message (conversations.star).
       added_in: 2
       openspec_change: add-starring-events
   properties:
     - { event: conversations.conversation_starred, version: 1, property: conversation,
         type: record, of: conversations.conversation, required: true,
         classification: operational, description: The conversation afterwards. }
   coverage:
     - { kind: operation, name: conversations.star, events: [conversations.conversation_starred] }
   ```

   A patch may also `remove:` rows by key (`remove: { coverage: [{ kind: job, name: x.y }] }`).

2. **Apply and generate.**

   ```sh
   cargo run -p campbooks-tracking-plan -- apply tracking/patches/0002-starring.yaml
   ```

   `apply` checks the patched plan, rewrites the workbook in canonical form
   and regenerates the mirror and the code. After a hand edit of the
   workbook, run `generate` instead. A module's first event adds
   `crates/modules/<m>/src/tracking.gen.rs`; include it from `lib.rs`:

   ```rust
   /// The module's tracked events, generated from the tracking plan (ADR 0020).
   #[path = "tracking.gen.rs"]
   pub mod tracking;
   ```

3. **Track it** in the transaction of the change, filled from the module's
   own rows, after the change (so the snapshot is the record as it is now):

   ```rust
   let mut tx = state.db.begin_for(workspace).await?;
   /* the change, then read the record back */
   state
       .tracker
       .track(&mut tx, &tracking::ConversationStarred { workspace, conversation: snapshot })
       .await?;
   tx.commit().await?;
   ```

   In a job or event handler, `ctx.track(&event).await?` does the same in the
   job's transaction. Track only when something changed: a no-op tracks
   nothing.

4. **Mark it live** (`status: live`) in the same PR, with a test that reads
   the event back (below). The check fails on a live event nothing tracks, and
   on a planned one something does.

5. **Describe it in the OpenSpec change**: the `design` lists the tracked
   events added or changed and the Coverage rows (openspec/config.yaml).

## Coverage

Every mutating operation (in `openapi/openapi.json` and
`openapi/admin.json`), job, schedule and event handler the server composes,
the kernel's own maintenance included, gets one Coverage row: the events it
tracks, or why it tracks none (`maintenance, no action of anyone`). Reads
need no row; one may be added to record that they track nothing.

**Coverage is required, with no baseline.** It is part of the definition of
done: a PR that adds an operation, a job, a schedule or an event handler
adds its Coverage row (and its events, live and tested) in the same PR, or
CI fails. There is no allow-list to add yourself to.

- The plan check (`cargo run -p campbooks-tracking-plan -- check`, which
  needs no server build) fails on a mutating operation with no row, on a row
  naming an `operationId` that is in neither document, on an event that
  does not exist, and on a row with both events and an exemption, or
  neither.
- The server's coverage test (`crates/server/src/tracking_tests.rs`,
  `coverage_names_what_the_server_composes`) builds the real registry and
  catalogue. It fails on a row naming something the server does not
  compose, and on any operation, job, schedule or handler with no row.
- Policy ("Tracking plan changes come through a patch") fails a PR that
  changes `tracking/tracking-plan.xlsx` or `tracking/tracking-plan.json`
  without adding or changing a file under `tracking/patches/`.

### Reading the failure and fixing it

The coverage test's message lists each missing item as `<kind> \`<name>\``,
for example:

```text
2 mutating operation(s), job(s), schedule(s) and handler(s) have no Coverage row. …
  - operation `things.make`
  - job `things.send_reminder`
```

The plan check prints the same for operations (`Coverage: operation
\`things.make\` mutates and has no row: …`). The kind is the row's `kind`
and the name its `name`: an `operationId`, or the `NAME` of the job,
schedule or handler. For each one:

1. Decide what it changes, and for whom. A change someone made, or a job
   or handler made on their behalf (a sync, a send, a rule applied), is an
   event, and so is a failure that matters to someone (with the kind of
   failure as an enum). Maintenance (pruning, re-tagging, refreshing a
   projection), a projection of an event tracked elsewhere, and work that
   only enqueues other work that tracks get an exemption that says so
   (`maintenance, no action of anyone: …`, `projection of <event>`,
   `consequence of <event>, tracked there`).
2. Add the row, with its events if any, to a patch
   (`tracking/patches/<nnnn>-<slug>.yaml`, the next free number on `next`
   and in open PRs), apply it, and track the events as in "Adding an
   event":

   ```yaml
   coverage:
     - { kind: operation, name: things.make, events: [things.thing_made] }
     - { kind: job, name: things.send_reminder, exemption: "only enqueues the sends, which track" }
   ```

3. Run `cargo run -p campbooks-tracking-plan -- check` and commit the
   patch, the workbook, the mirror and the generated code. The server's
   test runs in CI.

A row naming something that no longer exists (an operation renamed, a job
removed) fails too: rename the row, or remove it with a patch's `remove:`.

## The envelope and the ambient context

The kernel writes the envelope around each body; callers never set it:

| Field | From |
| --- | --- |
| `id` | a new UUIDv7 (subscribers deduplicate on it) |
| `name`, `version` | the generated type |
| `occurred_at` | the time it was tracked |
| `workspace_id` | the event (checked against the transaction's) |
| `actor`, `on_behalf_of` | the context: `{type, id}`, ids only. A `client` carries its client id only when Campbooks minted it (`campbooks-web`, `campbooks-native`, `cb_…`, `dcr_…`, `app_…`; `tracking::minted_client_id`); any other client, such as one known by its metadata document (a URL), is `{"type": "client"}` with no id |
| `operation` | the `operationId` being served |
| `correlation` | the request's trace id (or a fresh one), carried into every job and delivery it causes |
| `causation` | the job, or the event a delivery delivers |
| `role`, `app_version`, `plan_version` | the server |

The context is set by:

- **HTTP**: a layer around every catalogued route (`operations::compose`,
  and the server's admin, BFF and issuer routes) opens it with the matched
  operation, as `anonymous`; `Scoped` / `Workspace` then name the caller
  (`user` with the Campbooks account id, `client` with the client id) and
  `Admin` the administrator (`admin`, account id).
- **Invoked operations**: the assistant and automations are `assistant` /
  `automation`, `on_behalf_of` the person, with the enclosing correlation.
- **Jobs**: enqueueing (and fanning out a delivery) stores the context's
  person and correlation on the job row (`kernel.jobs.origin`, ids only); the
  worker runs the job as `system` (its kind), `on_behalf_of` that person, with
  that correlation, caused by the job or the event.
- **Outside any** (a CLI command): `system` with the command's name.

## Where events go

`CAMPBOOKS_TRACKING__SINK`:

- `none` (default): `track` returns at once and writes nothing.
- `outbox`: events are appended to `kernel.tracked_events` in the
  transaction of the change, and stay there (for tests and inspection). The
  runtime role may only append.
- `amqp`: the outbox, published to RabbitMQ by the relay
  (campbooks-cloud#321).

### The relay

With the sink `amqp`, every `worker` (and `all`) process runs a relay
(`kernel::tracking::Relay`); `api`-only processes only write the outbox.
Each batch is one transaction in the kernel scope:

1. claim up to `BATCH_SIZE` unpublished rows, oldest id first, locked
   `FOR UPDATE SKIP LOCKED` (`kernel.claim_tracked_events`), so several
   relays never hold the same row;
2. publish each row as one message (`mandatory`) and wait for every
   publisher confirm, at most 30 s;
3. mark the rows published (`kernel.mark_tracked_events_published`) and
   commit.

A nack, a message returned as unroutable, a broker error or the timeout
rolls the batch back, and its rows go out with a later one. A full batch is
followed by the next at once; otherwise the relay waits for the outbox's
`NOTIFY kernel_tracked_events` (an insert trigger) or `POLL_INTERVAL_MS`.
While the broker is unreachable, events wait in Postgres and the relay
reconnects with backoff (1 s up to 60 s, with jitter), logging the kind of
failure only: never the URL or its credentials. On shutdown a batch in
flight gets the worker's grace period (`CAMPBOOKS_JOBS__SHUTDOWN_GRACE_SECONDS`).

Delivery is **at least once**: a crash between the confirms and the commit
publishes a batch twice. Subscribers deduplicate on the event id. Events
from concurrent transactions can commit out of order, so subscribers order
by `occurred_at`.

The hourly `kernel.prune_tracked_events` deletes rows published more than
`PUBLISHED_RETENTION_HOURS` ago. It never deletes an unpublished row, however
long the broker is away. A purged workspace's rows, published or not, go
with its purge.

### The message

- Exchange `EXCHANGE` (topic, durable); routing key `<name>.v<version>`,
  e.g. `conversations.conversation_archived.v1`.
- Body (JSON): the envelope's fields at the top level, the event's
  properties and records under `body`:

  ```json
  {
    "id": "0192…", "name": "conversations.conversation_archived", "version": 1,
    "occurred_at": "2026-10-24T09:00:00Z", "workspace_id": "…",
    "actor": {"type": "user", "id": "…"}, "on_behalf_of": null,
    "operation": "conversations.archive", "correlation": "…", "causation": null,
    "role": "all", "app_version": "0.1.0", "plan_version": 1,
    "body": {"conversation": {"id": "…", "status": "archived", "…": "…"}}
  }
  ```
- Properties: `message_id` the event id, `type` the name, `content_type`
  `application/json`, `timestamp` when it happened (seconds), persistent,
  and the header `x-event-version`.

### Topology and how to subscribe

With `DECLARE_TOPOLOGY` on (the default), the relay declares at start, idempotently:
the exchange `EXCHANGE`, a stream queue `STREAM` (`x-queue-type: stream`,
`x-max-age: STREAM_MAX_AGE`), and a binding on `#`. Off, it declares
nothing and only publishes, which suits a broker user that may write but
not configure (the hosted cloud declares the topology with its
infrastructure, campbooks-cloud#322).

A subscriber that needs history reads the stream: over AMQP 0-9-1, consume
`STREAM` with a prefetch (`basic.qos`), manual acks and the argument
`x-stream-offset` (`first`, `last`, `next`, an offset, or a timestamp); or
with a stream protocol client. A subscriber that needs work-queue behaviour
binds its own quorum queue (with a dead-letter queue) to `EXCHANGE`, on `#`
or on the routing keys it wants.

### Settings

| Variable (`CAMPBOOKS_TRACKING__…`) | Default | |
| --- | --- | --- |
| `SINK` | `none` | `none`, `outbox` or `amqp` |
| `AMQP_URL` | | `amqp://user:password@host:5672/%2f` or `amqps://…`; required with `amqp`. Secret |
| `AMQP_PASSWORD_FILE` | | a file holding the broker password (a trailing newline dropped); it replaces the URL's, so the URL can omit it. Missing or empty stops the start |
| `EXCHANGE` | `campbooks.events` | the topic exchange |
| `STREAM` | `campbooks.events` | the stream queue declared and bound to it |
| `STREAM_MAX_AGE` | `30D` | how long the stream keeps events (`Y`, `M`, `D`, `h`, `m`, `s`) |
| `DECLARE_TOPOLOGY` | `true` | declare the exchange, stream and binding at start |
| `BATCH_SIZE` | `200` | events per batch (1 to 5000) |
| `POLL_INTERVAL_MS` | `1000` | how often an idle relay looks when no notification arrives |
| `PUBLISHED_RETENTION_HOURS` | `24` | how long published rows stay in the outbox |

Metrics (no ids): `campbooks.tracking.published{outcome=confirmed|nacked|error}`,
`campbooks.tracking.backlog` (rows waiting) and
`campbooks.tracking.oldest_unpublished` (seconds), sampled every 15 s while
metrics are exported.

## Testing

```rust
// The kit or state writes the outbox (the conversations kit: `Kit::new(db).await.tracking()`).
let state = campbooks_kernel::testing::tracking_outbox(state);
// … run the operation …
let events = campbooks_kernel::testing::tracked_events(&db, "conversations.conversation_archived").await;
assert_eq!(events[0].envelope["actor"], json!({"type": "user", "id": "sam"}));
```

`tracked_events` needs the test database's owner pool (the one
`#[sqlx::test]` hands you): the runtime role cannot read the outbox.

## The tool

```sh
cargo run -p campbooks-tracking-plan -- check                      # what CI runs
cargo run -p campbooks-tracking-plan -- generate                   # mirror + every tracking.gen.rs
cargo run -p campbooks-tracking-plan -- fmt                        # canonical workbook
cargo run -p campbooks-tracking-plan -- apply tracking/patches/…   # patch, fmt, generate
cargo run -p campbooks-tracking-plan -- init                       # a new, empty workbook
cargo xtask tracking [--check | generate | fmt | apply <patch>]    # the same, from xtask
```

The workbook the tool writes is byte-stable (rows sorted, fixed styles, the
creation date set to the plan version's date), so rewriting an unchanged plan
changes nothing. `check` compares parsed content, never bytes, so a workbook
saved by Excel or LibreOffice passes as long as its rows are right.
