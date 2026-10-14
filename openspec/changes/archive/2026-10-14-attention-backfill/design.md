# Design

## Data ownership and events

- **`conversations.needs_backfills`** (schema `conversations`): one row per
  inbox, `inbox_id` (primary key, cascades with the inbox), `workspace_id`,
  `state` (`pending`, `running`, `done`), `after_id` (the page cursor),
  `refreshed`, timestamps. Forced row-level security per workspace, plus two
  narrow maintenance policies (`SELECT` and `UPDATE`) for transactions that
  set no workspace: the dispatcher reads ids and states and starts rows,
  and sees no content. Granted to the runtime role.
- **Jobs**: `conversations.dispatch_needs_backfills` (scheduled every
  minute, queue `maintenance`, no workspace) and
  `conversations.backfill_needs` (one page, queue `maintenance`, priority
  -10, in the inbox's workspace, unique per inbox and cursor).
- **Events**: `conversations.need_changed` v1, published by
  `needs::refresh` in the page's transaction exactly as for live mail.
  `attention.project_conversation_need` projects it (ordering by revision
  already makes duplicate and out-of-order deliveries harmless).
- **No endpoint, scope or AI use.** No role is asked; there is nothing to
  undo beyond what members already have (Done, Not needed, Later).

## How a mechanism runs with no manual step

| Option | Runs on upgrade | New machinery | Trade-off |
| --- | --- | --- | --- |
| A. Migration-seeded work list + scheduled dispatcher + paged job (chosen) | yes: the migration lists the inboxes, the next tick starts them | one table, two job kinds, using the existing `schedule` and job queue | The dispatcher ticks forever (one indexed query on a tiny table a minute, like `connectors.dispatch_due`) |
| B. A kernel one-shot job enqueued by the migration | would be ideal | none | Not possible: a module migration touches only its own schema (`cargo xtask arch`), and `kernel.jobs` is the kernel's |
| C. A kernel "on boot" hook per module | yes | a new kernel concept for one use | Kernel API growth for a one-off; still needs to know which workspaces to visit |
| D. Start lazily on a workspace's first activity (as search does) | only once a member or a sync touches the workspace | none | A read path (Home, the inbox) would write, and an idle mailbox would wait indefinitely |
| E. A new event consumer replaying `message_received` | yes, by replay | a consumer | Replays every message ever stored and depends on the event log still holding them (it is pruned after 90 days) |
| F. An operator CLI | no | a command | Self-hosters must never run manual steps |

A wins: it runs on its own within a minute of starting the upgraded server,
uses only existing machinery, keeps each page in the inbox's workspace under
row-level security, and is resumable (each page records its cursor and
enqueues the next one, as `search.backfill` does). It does not copy the old
Rails app, which had no equivalent.

**Listing the inboxes in the migration.** Migrations run as the schema
owner. `FORCE ROW LEVEL SECURITY` applies to the owner too, and no workspace
is set in a migration, so a plain `SELECT` from `conversations.inboxes`
would see nothing unless the login is a superuser. The migration lifts the
force on `inboxes` for its one `INSERT … SELECT` and puts it back, inside
its own transaction (tested as a non-superuser owner). Row-level security on
`needs_backfills` is enabled after the seed.

## Which conversations, and what they get

- **The window.** The live needs window
  (`CAMPBOOKS_CONVERSATIONS__NEEDS_WINDOW_DAYS`, default 14) and not the
  decide backfill window or a new setting: `needs::refresh` already refuses
  to raise from a newest inbound message older than it, so a wider
  backfill window would only cost reads, and a separate setting would let
  the two disagree. The attention spec's own age rule (items more than 7 days
  past due are not listed) still applies on Home.
- **Open needs.** A conversation with an open need is refreshed whatever
  its age, so a need its stored answers no longer support (a question raised
  against a settled answer, or later one resting only on a model answer the
  module no longer trusts) is withdrawn as `superseded`.
- **Evidence only.** `needs::raise` reads stored decisions: `pay`, `reply`
  and `do` from answers the policy accepted, the phishing warning first,
  questions only from a model answer the policy did not accept and nothing
  acted against. Decisions stored before the sentence was kept give a need
  with no quote; deadline words still resolve to a date from the words
  alone ("Friday", "16/10"). No model or rules call is made: the tests run
  the backfill with rules that panic and a model server that must receive
  nothing.
- **Settled like live needs.** Replied conversations get no Reply or Do;
  archived ones keep Do and Pay but not Reply or Review; trashed ones get
  nothing; a member's Done stays done. A need that is cleared before it was
  ever open is not announced (no tombstones from the backfill).

## Restarting it later

The migration's seed statement resets existing rows to `pending`
(`ON CONFLICT … DO UPDATE`). A later change that alters how needs are worked
out (for example, model answers becoming inert) re-derives every inbox's
needs by running the same statement in its own migration. Because
`refresh` publishes only changes, a restart costs reads, not events.

## Estimate

On the demo mailbox the recordings use (the fake Gmail: 13 messages in 11
conversations, all within the window, synced by a server that raised no
needs), the backfill refreshed 11 conversations and raised 6 needs: one
phishing warning, three replies (two with due dates), one request and one
bill. The other five (a newsletter, receipts, notifications, a calendar
invitation) ask nothing under the rules. So a mailbox gets roughly one need
per person-to-person question, request or bill in the last 14 days; list
and service mail, most of a typical inbox, raises none. For the production
instance this means its recent person-to-person mail fills Home, not its
history.
