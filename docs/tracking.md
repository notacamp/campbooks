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
| The kernel | `campbooks_kernel::tracking` | `Tracker::track`, the ambient context, the outbox `kernel.tracked_events`. |

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

Every operation, job, schedule and event handler gets one Coverage row: the
events it tracks, or why it tracks none (`maintenance, no action of
anyone`). Reads need no row; one may be added to record that they track
nothing.

- The plan check fails on a row naming an `operationId` that is in neither
  `openapi/openapi.json` nor `openapi/admin.json`, on an event that does not
  exist, and on a row with both events and an exemption, or neither.
- The server's coverage test (`crates/server/src/tracking_tests.rs`) builds
  the real registry and catalogue and fails on a row naming an unknown job,
  schedule or handler.
- Both list what has no row yet as warnings. Once every module is
  instrumented (campbooks-cloud#332) that becomes a failure
  (`REQUIRE_FULL_COVERAGE`).

## The envelope and the ambient context

The kernel writes the envelope around each body; callers never set it:

| Field | From |
| --- | --- |
| `id` | a new UUIDv7 (subscribers deduplicate on it) |
| `name`, `version` | the generated type |
| `occurred_at` | the time it was tracked |
| `workspace_id` | the event (checked against the transaction's) |
| `actor`, `on_behalf_of` | the context: `{type, id}`, ids only |
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
  transaction of the change. The runtime role may only append; nothing reads
  them back yet. Publishing them to RabbitMQ (`amqp`) is campbooks-cloud#321.

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
