# Design

The decision and its alternatives are in ADR 0020. This file covers how it
is built.

## Data ownership

- `kernel.tracked_events`: the outbox. Columns: `id uuid` (UUIDv7, the
  event id), `workspace_id` (nullable for instance-wide events), `name`,
  `version`, `occurred_at`, `envelope jsonb` (actor, on behalf of,
  operation, correlation, causation, role, app and plan version), `body
  jsonb` (properties and record snapshots), `published_at` (null until the
  relay confirms it, #321). Row-level security matches `kernel.events`: a
  workspace transaction appends only its own workspace's rows, and the
  runtime role can't read or change them back (the relay reads through a
  definer function in #321). Migration `<ts>_kernel__tracked_events.sql`.
- `kernel.jobs.origin jsonb` (nullable, ≤ 1 KB, ids and enums only): the
  ambient actor, on behalf of and correlation when the job was enqueued or the
  delivery fanned out. Migration `<ts>_kernel__job_origin.sql`. The payload
  audit (`xtask payloads`) lists its fields.

## The plan workbook

| Sheet | One row per | Key columns |
| --- | --- | --- |
| README | — | how to edit, naming rules, the classification table, plan version |
| Events | event version | name, version, module, object, action, description, fires when, actor types, status (`planned`/`live`/`deprecated`), added in, OpenSpec change |
| Properties | event property | event, version, property, type, enum or record, required, classification, description, example |
| Records | snapshot field | record type (`<module>.<record>`), field, type, enum, classification, description |
| Enums | enum value | enum, value, description |
| Actors | actor type | type, id is, description (fixed rows: user, client, assistant, automation, system, admin, anonymous) |
| Coverage | operation / job / schedule / handler | kind, name, events (comma-separated), exemption reason |
| Changelog | plan change | date, plan version, change, PR |

Property types: `id`, `record_ref`, `record` (a snapshot from Records),
`enum`, `bool`, `int`, `decimal`, `duration_ms`, `timestamp`, `date`,
`bytes`, `currency`, `country`, `locale`, `code` (needs a pattern). Lists of
any type except `record` are written `list<type>`. Classifications: `id`,
`pseudonymous`, `operational`. `personal` and `content` are valid for
writing down what a field *would* be, and the check rejects them, so a
rejected field shows up in review instead of being dropped without notice.

## Checks (`cargo xtask tracking --check`)

1. The workbook parses; sheets and headers are as above; dropdown values are
   in their sets.
2. Names follow `<module>.<object>_<past_verb>`, the module exists, and
   `(name, version)` is unique. Only the newest version of an event may be
   `live` alongside `deprecated` older ones.
3. Every property and record field has an allowed type and classification;
   `code` has a pattern; enums and records referenced exist; nothing is
   `personal` or `content`.
4. The mirror and every generated file equal what the workbook produces.
5. Every `live` event's generated type is used outside `*.gen.rs` and tests
   in its owning crate; a `planned` one isn't (it must become `live`).
6. Coverage: each row names an existing `operationId` (from `openapi.json` or
   `admin.json`) or a registered job, schedule or handler kind, and the
   events it lists exist. Each row lists events or an exemption, not both.
   Unmapped operations and jobs are reported as warnings until #332 makes
   them errors with `--require-coverage`.

## Generated code

One `tracking.gen.rs` per owning crate (`crates/modules/<m>/src/` or the
kernel for kernel- and server-owned events), included by a hand-written
`mod tracking;`. Per event: a struct with typed fields (snapshots as
generated `<Record>Snapshot` structs, enums as generated enums) and an
`impl TrackedEvent` carrying `NAME`, `VERSION`, `MODULE` and whether it is
workspace-scoped. Modules fill snapshots from their own rows. The generated
structs can only be built with every required field.

## The ambient context

`TrackingContext` is a tokio task-local:

- **HTTP**: a layer after authentication sets `actor` from the `Principal`
  (`user` with the account id, `client` with the client id), `operation` from
  the matched route's `operationId`, and `correlation` from the request's
  trace id. Admin routes set `admin` with the admin profile's account.
  Unauthenticated routes (sign-up, sign-in) set `anonymous`, with no id.
- **Assistant and automation** wrap the operations they invoke on someone's
  behalf: actor `assistant` / `automation`, `on_behalf_of` the person.
- **Jobs**: the worker sets actor `system` (id = job kind), `on_behalf_of`
  from `origin.actor` when present, `correlation` from `origin`, and
  `causation` = the job id, or for a delivery, the event id.
- **Outside any context** (CLI, startup), `track` uses `system` with the
  command's name. It never fails because the context is missing.

`Tracker::track(&mut tx, &E)` checks that the transaction's workspace
matches the event's (as `EventBus::publish` does), builds the envelope from
the context, and inserts the row. With `SINK=none` it returns at once
without touching the database.

## Reference flow

`conversations.archive` (and its undo, `conversations.unarchive`) track
`conversations.conversation_archived` / `conversations.conversation_unarchived`
with a `conversations.conversation` snapshot (stream, status, message
count, unread count, has attachments, last activity, inbox id), all in the
handler's transaction. The Coverage sheet maps both operations.

## Events published or consumed

None. This change adds tracked events, not domain events.
