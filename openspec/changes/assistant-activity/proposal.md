# Proposal

## Why

The owner asked for "a page of logs/sessions where we see what scout is
doing and can see thinking/decision logs", and for it to be **real time**:
the page updates while the assistant works, with its thinking and decisions
streaming in, not a static history.

Today the decide pass is invisible while it runs. Its answers are stored
(`conversations.decisions`: value, probabilities, confidence, model, reasons,
whether it acted), but a member sees them only one conversation at a time,
in the reader's note and its "Why?", after the fact. Nobody can see that 12
new messages are being sorted, which one the model is on, what it suggested,
what was applied and what was only a suggestion, or that the pass failed.
ADR 0009 makes every decide answer a suggestion until an eval shows it can
act, so seeing and correcting what the assistant thought is how members
build trust in it and how the corrections that recalibrate it (#59) get
made.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/89.
Related: #47 (assistant module), #59 (decide quality, corrections), #65
(event retention and purge), #448 (operator tracing, separate from this),
#453 (change feed), #459 (inbox suggestions with "Why?").

Modules touched: a **new module `activity`** (server crate, schema
`activity`, client module), **`conversations`** (records its triage passes,
one new read operation), the **kernel** (a new port, `Activity`, like
`Visibility` and `GrantIntake`), the app shell (navigation) and `lib/ui`.
No cross-module dependency: producers reach `activity` only through the
kernel port, and the new module needs no `architecture.toml` entry. The port
is recorded in a new ADR (0015) in this change.

## What Changes

- **Sessions and steps.** A *session* is one unit of assistant work with one
  cause and one audience: today, the triage of one sync batch of one inbox;
  later a draft, a chat turn, a rule run, an extraction. It has a kind,
  trigger, status (running, done, failed, stalled), start and end, the roles
  and models it used, token counts, the records it is about (*subjects*), and
  a *timeline of steps*. A step is one stage of work on one subject (rules,
  nearest corrections, the `decide` model; later `complete`), with its state
  (working, done, skipped, failed), strategy, model, latency, tokens, a small
  set of non-personal facts (the question ids, the answer labels and
  numbers, confidence, probabilities, whether it acted, reason rule ids) and
  references to the records it produced (`conversations.decision`).
- **The `activity` module** owns sessions and steps in schema `activity`. It
  holds **no personal data**: no subjects, senders, bodies, evidence or
  prompts, only ids, labels and numbers. What a member reads about a message
  (its subject, sender, the evidence behind each reason, corrections) stays
  in `conversations` and is fetched from it under its own access rules.
- **Kernel port `Activity`**: `record(state, conn, entry)` opens or continues
  a session and records steps in the transaction the producer passes, so a
  decision and its step commit together. A producer records a step as
  *working* in a short transaction of its own before a slow call (so the page
  shows what is in progress) and as done in the transaction that stores the
  result. The default port discards everything, so modules and their tests
  run without `activity`.
- **`conversations` records triage.** `apply_changes` opens a
  `conversations.triage` session per batch with one rules step per inbound
  message, and marks the model stage pending for each message the model will
  read; `conversations.decide_message` records its model step against the
  session that covers the message (working before the call, done, skipped or
  failed after it).
- **Live**: every recorded entry publishes `activity.session_changed` (ids
  only), which declares an upsert of `activity.session` within the session's
  container (the inbox) to the change feed (#453). Clients refetch the
  session and only the steps after the last one they hold. No second
  streaming endpoint.
- **New read operations** (`activity:read`): `activity.list_sessions` (newest
  first, cursor, status filter) and `activity.get_session` (the session and
  its steps, `after` a step for incremental reads). Sessions in an inbox the
  caller is not a member of are omitted from lists and 404 when read.
- **New read operation** `conversations.list_decisions` (`conversations:read`):
  for up to 100 messages, their header (subject, sender, stream and who
  placed it), every answer of every strategy with its reasons and evidence,
  and the corrections on record. The activity page's "Why?" reads it.
- **Retention and purge**: sessions and steps are deleted after
  `CAMPBOOKS_ACTIVITY__RETENTION_DAYS` (default 30), with the workspace when
  it is purged, and a departed member's private sessions when they leave.
  Steps left working past `CAMPBOOKS_ACTIVITY__STALL_MINUTES` (default 15)
  are marked stalled.
- **The page.** "Activity" in the sidebar (G then A), with a live Visor as
  its icon that shows what Campbooks is doing anywhere in the workspace; on
  phones the Visor sits in the top bar. `/w/$workspaceId/activity` lists
  sessions as they start and finish ("Sorting 12 new messages in Work"),
  with All / Running / Needs a look; a session opens beside the list
  (desktop) or as its own screen (phone) with its steps streaming in, each
  message's answers as suggestions or as applied, "Why?" with the reasons,
  and Open conversation (O). The Visor's existing states carry the live
  status (reading, thinking, unsure, happy, alert, asleep). en, pt-PT,
  es-ES, fr; 375px and desktop; keyboard first.
- **ADR 0015** (assistant activity is a personal-data-free journal behind a
  kernel port, live through the change feed); `docs/domain-map.md` and
  `docs/architecture.md` gain the `activity` context.

## Capabilities

### New Capabilities

- `activity`: sessions and steps of assistant work, how producers record
  them, who may read them, live updates, retention and purge.
- `activity-ui`: the Activity page, the live Visor in the shell.

### Modified Capabilities

- `conversations`: its triage passes are recorded as activity; it serves the
  answers and corrections of a set of messages.

## Non-goals

- **Corrections from the page** ("This was wrong" for every triage
  question): the next change, `decision-corrections`, adds the
  `conversations` operations for questions other than the stream and the
  control on this page and in the reader. This change links each answer to
  its conversation, where moving it to another stream already records a
  correction.
- **Reasoning text and token streaming** for the `complete` and `vision`
  roles (drafts, extraction, chat turns). No module uses those roles yet;
  the design fixes the contract (stored capped and without prompts, on by
  default, with a workspace switch to turn it off), and the change that adds
  the first such producer (#47 or the drafting slice) builds it.
- **New Visor emotions** (dizzy, starry, heart eyes) and the reworked idle,
  reading and thinking motion (option B) from the owner's prototype: the
  separate design-system change `visor-emotions`. This page uses `alert` for a failed session until
  `dizzy` exists, then switches with a one-line mapping.
- Recording work of modules that do not exist yet (tasks, documents, money,
  automation, assistant): each records its sessions in the change that
  builds it.
- Operator tracing and metrics (#448): spans, logs and metrics stay the
  operator's and carry no personal data or ids; this page is for members.
- Backfilling sessions for mail sorted before this ships.
- Money cost: tokens are counted; prices per model are not in the catalogue.
- `conversations.message_decided` (the domain map's `MessageDecided`): the
  tasks module's change adds it with its first consumer.
- Cancelling a running session, re-running a pass, or a cross-workspace
  admin view.
