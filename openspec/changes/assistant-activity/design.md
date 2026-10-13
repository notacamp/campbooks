# Design

## Owner decisions (2026-10-13)

The owner approved this design with three answers:

1. **Where the page lives:** a sidebar item "Activity" (G then A) whose icon
   is the live Visor; on phones the Visor sits in the top bar (Decision 5).
2. **Visor emotions:** dizzy, starry and heart eyes ship in their own
   design-system change, `visor-emotions`, built separately, with thinking
   motion B (hopping eyes). This page shows `alert` for failures until that
   lands, then uses `dizzy` (Decision 5).
3. **Retention and reasoning:** the journal is kept 30 days by default.
   Reasoning text from the `complete` role is stored when it arrives, capped
   and never with prompts, and a workspace switch turns storing it off
   (Decision 2; built in the reasoning slice).

## Owner feedback (2026-10-13): paused until Scout has real jobs

The owner reviewed the prototype and paused this change. The feedback:

1. The dizzy face is not acceptable: its eyes are bigger than the face.
   That is fixed in the `visor-emotions` change, not here.
2. Every example was sorting, and a log of sorting alone is of little use.
3. An opened session was hard to understand: it read as a log of stages,
   not as what happened.

**Decision.** Scout gets real jobs first: Home showing what needs the member
(replies owed, deadlines, asks, suspicious mail), then drafts and summaries.
The Activity page is redesigned around those jobs afterwards, and it is
**built only once those jobs exist**. The data model, the kernel port, the
live transport and the access rules above still stand; the page design
(Decision 5) and the slicing (Decision 6) are to be redone.

**What the redesign must do.**

- **Each entry reads as a plain sentence first**, about the person and the
  thing, not the pipeline. For example:
  - "Marta asks you to confirm Thursday's walkthrough. Scout kept it in
    Primary and marked it Needs a reply, due Thursday."
  - "Acme Hosting's invoice INV-2041 is due on 30 October. Scout added the
    deadline to Home."
  - "Scout drafted a reply to Jonas about lunch on Friday. It is waiting in
    your drafts for you to read and send."
  - "IT Support asks you to verify your mailbox, from a look-alike of your
    own domain. Scout flagged it as suspicious and kept it out of Home."
  - "Scout summarised the 14-message thread about the studio repaint:
    Pedro's quote, the two dates offered, and that he is waiting for your
    answer."
  - "Scout sorted 6 new messages in Work: 1 newsletter moved, 4 left where
    they were."
- **Sorting is one job among several**, never the whole page. Entries span
  Scout's jobs: noticed a deadline, found an ask, drafted a reply, flagged
  phishing, summarised a thread, sorted mail.
- **How it decided is folded away.** Stages, strategies, confidence,
  probabilities and thresholds sit behind "How Scout decided" on each entry,
  closed by default. What the member reads first is what Scout did, about
  whom, and what is left for them to do (with the action to do it).
- **Live updates stay**, now expressed per job: "Scout is drafting a reply
  to Marta" while a step works, then the sentence once it lands.

The name in UI copy follows the product's copy rules when the page is
built; these examples use the owner's wording.

## Context

- **ADR 0009**: `decide` is a typed decision model (Laya by default). Its
  answers are labels, levels or yes/no with a probability per option and a
  confidence, never text. Until a question's eval shows it works, every
  answer is a suggestion. Strategies (`campbooks_ai::decide::strategy`:
  email signals, k-NN over corrections, the model, a cascade) all return the
  same `Answer` plus `Reason { rule, evidence }`.
- **Where the decide pass runs today.** `conversations.apply_changes` (queue
  `sync`, one job per connector batch, ordered per inbox) stores each new
  message and asks the rules strategy in the same transaction (source
  `rules`). `conversations.decide_message` (queue `ai`, an event consumer of
  `conversations.message_received`, one job per inbound message) asks the
  model (source `model`) when the `decide` role is configured and the
  message is inside the backfill window. Both write `conversations.decisions`
  (value, options, probabilities, confidence, model, latency, reasons,
  `acted`, `state_version`), unique per message, question and source.
  Moving a conversation records `conversations.corrections`.
- **ADR 0011 / 0012**: events go through the outbox in the producer's
  transaction and fan out to jobs; `kernel.events` is retained (until #65)
  and carries ids and non-personal facts only. A job's `ctx.tx()` commits
  with its completion, so anything it writes there is invisible until the
  job ends.
- **Change feed (#453)**: an event's `changes()` (ids only, optionally within
  a container) reach clients as SSE frames, filtered by the scopes of the
  read operations that return the record type and, for a container, by the
  kernel's `Visibility` port (implemented by `conversations` for inboxes).
  Clients invalidate TanStack queries from `records.gen.ts`. Sequencing adds
  one job hop (tens to hundreds of milliseconds).
- **Telemetry (#448)**: spans and metrics never carry personal data, and
  metric attributes never carry ids. Operator traces are sampled, off by
  default and pruned by the collector: not a product surface.
- **Inbox UI (#459)**: the reader's Note shows the answers as suggestions and
  "Why?" lists the reasons, translated client-side by rule id
  (`conversations:reasons.<strategy>.<rule>` with `{{evidence}}`).
- **The Visor** (`lib/ui`): idle, reading, thinking, unsure, happy, alert,
  asleep; sm to xl; motion off under reduced motion; `announce` reads the
  state out ("Campbooks is reading"). User-facing copy calls the assistant
  "Campbooks"; "Scout" is an internal word.

## Decision 1: what a session is, and where it is recorded

### What a session is

A **session** is one unit of assistant work that has **one cause** and **one
audience**. A **step** is one stage of that work on one subject. Granularity
options for triage:

| Option | A sync batch of 12 messages shows as | Cost |
| --- | --- | --- |
| (a) One session per job run | 1 rules session + 12 model sessions, grouped by the client by time and inbox | Simplest to record; the list is noisy and grouping is guesswork |
| (b) **One session per cause (the batch)**, later stages join it by subject | 1 session, 12 messages, 24 steps | The model job must find the batch's session |
| (c) One session per message | 12 sessions | Nothing to group; the list is a copy of the inbox |

**Chosen: (b).** It matches how the owner describes the work ("a decide pass
over a sync batch") and what a member wants to see ("sorting 12 new
messages"). The model job does not know its batch, and adding the batch to
`message_received` would change a published contract for the sake of a
display, so a later stage **joins the session that covers its subject**:
the rules stage lists each message as a subject with the stages still to
come (`pending: ["model"]` when the role is configured and the message is
inside the backfill window), and the model step names the subject and the
session kind (`conversations.triage`). The port attaches it to the newest
such session that still has that stage pending for that subject, or opens a
session of its own when none does (a replay, or a message whose session was
pruned).

A session is **running** while any subject has a pending stage or any step
is working, then **done** (its outcome counts subjects, suggestions, applied
answers, skips and failures). It is **failed** when the work as a whole
failed (a batch that could not be applied, recorded on the job's last
attempt in a transaction of its own) and **stalled** when a step stayed
working past `CAMPBOOKS_ACTIVITY__STALL_MINUTES` (a worker died mid-call).
A pending stage that has not started never stalls: the `ai` queue can be
hours behind during a backfill, and the page says "waiting its turn".

Other kinds follow the same shape when their modules arrive: a draft
(`conversations.draft`, audience: the member), a chat turn
(`assistant.turn`, audience: the member), a rule run (`automation.run`,
audience: the rule's container), an extraction (`tasks.extract`,
`documents.read`).

### Where it is recorded

| | A. **New `activity` module behind a kernel port** | B. Each module keeps its own activity rows | C. `activity` projects from domain events | D. Reuse job rows and traces |
| --- | --- | --- | --- | --- |
| Source of truth for the journal | `activity` schema | every producer's schema | the event log | `kernel.jobs`, the trace collector |
| One list across kinds, one cursor | yes | merged in the client from N endpoints | yes | no |
| Personal data | none (ids, labels, numbers) | per module | must stay out of events (#65), so content would travel as blobs per step | forbidden in telemetry |
| Atomic with the decision | yes (producer's transaction) | yes | yes (outbox) | no |
| Live "working" state | a short transaction before the call | per module | an event per step, plus a delivery job each | no |
| New machinery | a port, a module, one event type | N copies of sessions, steps, retention, live | consumers for every producer's step events | none, but it is pruned in hours and off by default |
| Self-host weight | one schema | none | the event log grows with every step forever (until #65) | none |

**Chosen: A.** It is the only option that gives one live list with one
access rule and one retention policy without putting step content into the
retained event log. B makes every module re-implement the same journal and
the client merge pages from N cursors. C is the textbook projection (ADR
0011) and would be replayable, but replaying "what the assistant was
thinking at 10:02" has no value, every step would be a retained event plus a
delivery job, and the content would still have to live somewhere else. D is
operator data: sampled, pruned within hours, and by rule free of anything a
member could recognise.

A **synchronous port** (rather than events) is justified the way `Visibility`
and `GrantIntake` are: the write must commit atomically with the producer's
own change, and the alternative is an event per step. The port is a kernel
trait; `activity` implements it and the server registers it, so no module
imports another. The default port (`NoActivity`) discards entries, so
producers and their tests never depend on `activity` being present.

We also considered **recording at the AI gateway** (every `decide`/`chat`
call becomes a step automatically, attached to "the current session" through
a task-local). It cannot record the rules and k-NN stages (no gateway call),
does not know what a call was about, and hides the session in ambient state.
Producers record steps explicitly and pass the latency and token usage the
gateway already returns.

### The port

```rust
// campbooks_kernel::activity
pub trait Activity: Debug + Send + Sync + 'static {
    /// Open or continue a session and record steps in `conn`'s transaction.
    /// Publishes `activity.session_changed` in the same transaction.
    fn record<'a>(&'a self, state: &'a AppState, conn: &'a mut PgConnection,
                  entry: Entry) -> ActivityFuture<'a, SessionId>;
}
pub struct Entry { pub workspace: WorkspaceId, pub session: Target, pub steps: Vec<Step> }
pub enum Target {
    New(SessionSpec),                                   // the rules stage of a batch
    Id(SessionId),                                      // a producer that kept the id
    Covering { kind: &'static str, subject: RecordRef,  // a later stage
               stage: &'static str, otherwise: SessionSpec },
    Failed(SessionSpec, &'static str),                  // whole-session failure, error code
}
pub struct SessionSpec {
    pub kind: &'static str,          // `<module>.<kind>`, checked against the producer
    pub trigger: Trigger,            // Sync | Member | Rule | Schedule | Assistant
    pub audience: Audience,          // Container(RecordRef) | Member(UserId) | Workspace
    pub started_by: Option<UserId>,
    pub subjects: Vec<Subject>,      // { record: RecordRef, pending: Vec<&'static str> }
}
pub struct Step {
    pub subject: Option<RecordRef>,
    pub stage: &'static str,         // rules | knn | model | cascade | (later) complete
    pub state: StepState,            // Working | Done | Skipped | Failed
    pub role: Option<&'static str>, pub strategy: Option<String>,
    pub model: Option<String>, pub model_version: Option<String>,
    pub latency: Option<Duration>, pub usage: Option<Usage>,
    pub facts: Facts,                // non-personal: see Decision 2
    pub refs: Vec<RecordRef>,        // e.g. conversations.decision ids
    pub error: Option<&'static str>, // a stable code, never a provider message
}
```

Kinds and stages are `&'static str` so a producer cannot pass text it read
from mail; `cargo xtask arch` rejects a kind not prefixed by the module that
records it.

### Data ownership: schema `activity`

```sql
CREATE TABLE activity.sessions (
    id uuid PRIMARY KEY,                 -- UUIDv7
    workspace_id uuid NOT NULL,
    kind text NOT NULL,                  -- conversations.triage
    trigger text NOT NULL CHECK (trigger IN ('sync','member','rule','schedule','assistant')),
    started_by uuid,                     -- a user id (workspaces), when a member caused it
    audience text NOT NULL CHECK (audience IN ('container','member','workspace')),
    container_type text, container_id text,  -- set iff audience = 'container'
    member_id uuid,                          -- set iff audience = 'member'
    status text NOT NULL CHECK (status IN ('running','done','failed','stalled')),
    outcome jsonb NOT NULL DEFAULT '{}',     -- counts: subjects, suggested, applied, skipped, failed
    roles text[] NOT NULL DEFAULT '{}', models text[] NOT NULL DEFAULT '{}',
    tokens_in bigint NOT NULL DEFAULT 0, tokens_out bigint NOT NULL DEFAULT 0,
    error text,                              -- a stable code
    started_at timestamptz NOT NULL, updated_at timestamptz NOT NULL, ended_at timestamptz
);
CREATE INDEX sessions_list_idx ON activity.sessions (workspace_id, started_at DESC, id DESC);
CREATE INDEX sessions_running_idx ON activity.sessions (workspace_id) WHERE status = 'running';

CREATE TABLE activity.subjects (
    session_id uuid NOT NULL REFERENCES activity.sessions ON DELETE CASCADE,
    workspace_id uuid NOT NULL,
    record_type text NOT NULL, record_id text NOT NULL,
    pending text[] NOT NULL DEFAULT '{}',    -- stages still to come
    PRIMARY KEY (session_id, record_type, record_id)
);
CREATE INDEX subjects_record_idx ON activity.subjects (workspace_id, record_type, record_id);

CREATE TABLE activity.steps (
    id uuid PRIMARY KEY,                     -- UUIDv7: timeline order and `after` cursor
    session_id uuid NOT NULL REFERENCES activity.sessions ON DELETE CASCADE,
    workspace_id uuid NOT NULL,
    subject_type text, subject_id text,
    stage text NOT NULL,
    state text NOT NULL CHECK (state IN ('working','done','skipped','failed')),
    role text, strategy text, model text, model_version text,
    latency_ms integer, tokens_in integer, tokens_out integer,
    facts jsonb NOT NULL DEFAULT '{}',       -- versioned, non-personal
    refs jsonb NOT NULL DEFAULT '[]',        -- RecordRefs
    error text,
    started_at timestamptz NOT NULL, ended_at timestamptz
);
CREATE INDEX steps_session_idx ON activity.steps (session_id, id);
CREATE INDEX steps_working_idx ON activity.steps (started_at) WHERE state = 'working';
```

A *working* step is finished by updating it (same session, subject and
stage), so the timeline shows one row per stage. Row-level security is
enabled and forced on all three tables with the usual
`workspace_isolation` policy; `campbooks_app` gets what it needs. Other
modules' records are referenced by `RecordRef` only.

### Events

- **Publishes** `activity.session_changed` v1 `{ workspace, session, status }`,
  once per `record` call, in the producer's transaction. `changes()` declares
  an upsert of `activity.session` within the session's container (or no
  container for workspace-wide sessions); a member-private session declares
  no container and is filtered by the read operations, as drafts are.
  Retained like every event (ids only); #65's pruning applies.
- **Consumes** `workspaces.workspace_deleting` (purge after the grace period,
  as `conversations` does) and `workspaces.member_left` (delete that
  member's private sessions).
- **Schedules**: `activity.prune` (hourly: sessions older than the
  retention), `activity.sweep_stalled` (every 5 minutes).

### Scopes and operations

| Operation | Route | Scope | `x-campbooks` |
| --- | --- | --- | --- |
| `activity.list_sessions` | `GET /api/v1/w/{workspace_id}/activity/sessions?status=&cursor=&limit=` | `activity:read` | query; records `activity.session`; assistant |
| `activity.get_session` | `GET …/activity/sessions/{session_id}?after=&limit=` | `activity:read` | query; records `activity.session`; assistant |
| `conversations.list_decisions` | `GET …/decisions?message=…` (1 to 100 ids) | `conversations:read` | query; records `conversations.message` |

`activity:read` is a new scope in `Module.scopes`; the web BFF requests every
catalogue scope, so the web app gets it at the next sign-in (an existing
session's token lacks it and the page says so; see Rollout).

### Does this copy the Rails app?

The session → steps shape resembles the Rails app's workflow executions
(an execution with one row per step and its input and output). The shape is
kept because it is the natural one for a timeline; the Rails design is not
copied: Rails stored each step's inputs and outputs (mail content) in the
execution rows, refreshed screens with Turbo Streams per user, and had no
container-level access rule. Here the journal holds no content, points at
the owning records, and goes live through the change feed. It wins on
privacy and on retention, not on familiarity.

## Decision 2: what "thinking" means per role

| Role / stage | What the assistant "thinks" | Stored in `activity` | Shown from the owner | Streamed only |
| --- | --- | --- | --- | --- |
| Rules (email signals) | which signals fired, per question | per question: answer, confidence, probabilities, acted, reason rule ids | evidence (the header, the phrase) from `conversations.list_decisions` | nothing |
| k-NN over corrections | the nearest corrected messages and their similarity | answer, confidence, neighbour count | the correction records | nothing |
| `decide` model (Laya or an instruct adapter) | a probability per option, per question | answer, probabilities, confidence, the threshold it was gated against, acted, model, latency | the same answers with their reasons | the step's *working* state while the call runs |
| `complete` / `vision` (later) | reasoning text when the model returns it, and the output | role, model, latency, token counts, a reference to the reasoning blob and to the output record | the output (a draft, a chat reply) from its module | the reasoning tokens while they are generated |

- **Decide produces no text and we invent none.** "Thinking" for the decide
  role is the stage-by-stage trace: which strategy answered which question,
  with what probability, against which threshold, and whether that changed
  anything. We do not ask a generative model to write a rationale after the
  fact: it would read as an explanation of a decision it did not make.
- **Facts are non-personal.** `facts` for decide stages is
  `{ "v": 1, "answers": [{ "question", "value", "options", "probabilities",
  "confidence", "threshold", "acted", "rules": ["signals.list_header"] }] }`:
  question ids, labels from the question's own option set, numbers and rule
  ids. Evidence is not copied: a matched phrase or a sender's domain is
  derived from mail and stays in `conversations`, read under inbox access.
  `conversations.decisions` remains the source of truth (and the eval
  set's); steps reference its rows.
- **Reasoning text (later slice), when stored:** capped at 16 KiB per step,
  kept as a blob attached to the step (content-addressed, deleted by the
  blob sweep when the step is pruned or the workspace purged), never in a
  Postgres column, never in an event, never in telemetry. Prompts are never
  stored: they are rebuilt from records the member can already read.
  The raw token stream is delivered live by the producing module's own
  stream (the assistant's chat stream, #47) and is not kept beyond the
  capped final text. Storing reasoning is **on by default** under the same
  30-day retention, and a **workspace setting turns it off** (owner
  decision); with it off, steps keep their facts, token counts and output
  references only.

Alternatives considered: copying each answer with its evidence into the step
(one fetch per session, but a second copy of mail-derived text with its own
access and retention rules); storing only decision ids and no facts (the
timeline could not render a line until `conversations` answers, and the list
could not count suggestions); storing full reasoning for every role (the
largest privacy surface, and no role produces it today).

## Decision 3: live transport

| | A. **Change feed (ids) + incremental fetch** | B. Dedicated per-session SSE with step content | C. Polling |
| --- | --- | --- | --- |
| New server machinery | none (one record type) | a second SSE stack: LISTEN channel, hub, per-caller limits, token-expiry close, resume cursor, BFF pass-through | none |
| Authorization | the feed's scope + `Visibility` filtering, then each fetch's own checks | content on the wire: checked at open and again whenever membership might change mid-stream | each fetch |
| Latency | one sequencing hop + one fetch (sub-second) | lowest | the poll interval |
| Resume after disconnect | the feed's cursor; a `reset` refetches | its own cursor per session | trivial |
| Token streaming | no | yes | no |

**Chosen: A for sessions and steps.** Steps are metadata about a few
seconds of work, not a token stream, and the feed already solves resume,
backpressure, limits, liveness and access filtering. A per-session stream
would duplicate it to save one fetch. **B is reserved for reasoning tokens**,
owned by the module that generates them, in the later slice.

How it behaves:

- Each `record` call publishes one `activity.session_changed`, so a batch's
  rules stage is one frame, and each model step is two (working, done).
- The client maps `activity.session` to `activity.list_sessions` and
  `activity.get_session` (generated `records.gen.ts`). It **coalesces
  invalidations for 250 ms** and fetches only steps `after` the last step id
  it holds (limit 200); a gap or `reset` refetches the session whole.
- Backpressure: producers never wait for readers; the feed reads in pages;
  a burst of 50 model steps becomes a handful of incremental fetches. The
  list never moves the member's selection: new sessions slide in above it
  and a "N new" pill appears when the selection is not at the top.
- Reconnect: the feed resumes from its cursor (`Last-Event-ID`); the page
  keeps rendering its cache while offline and marks the live dot as paused.
- A visibility change (the feed's `revoked`, a 404 on fetch) removes the
  session from the cache.

## Decision 4: privacy and access

- **Who sees a session.** Its audience decides:
  `container` (an inbox): members of that inbox, through the kernel's
  `Visibility` port (implemented by `conversations`), the same rule the
  change feed and every conversations read apply; a client token bound to
  the workspace sees every inbox, as it does over the API.
  `member`: that member only (drafts, chat turns); never client tokens.
  `workspace`: every member (work that touches no container).
  Workspace admins get no extra view: an inbox's activity is as private as
  its mail.
- **Reads that may not be seen answer 404**, and lists omit them. The list
  asks the port once per distinct container in the workspace (a handful of
  inboxes) and filters in SQL, so pages are never short.
- **One container per session.** A producer that works across inboxes opens
  one session per inbox (a batch is per inbox already). Every subject of a
  session lives in its container; `conversations` tests that.
- **Tenancy.** `workspace_id` on every row, queries filter on it, RLS forced
  on all three tables, `begin_for` for requests and `ctx.tx()` for jobs.
- **No personal data, no secrets.** Kinds and stages are static strings;
  facts are ids, option labels from the question set, numbers and rule ids;
  errors are stable codes (`provider_unavailable`, `rate_limited`,
  `timeout`, `invalid_answer`, `stalled`), never a provider's message (the
  existing `redact::ai` already maps them). Model names come from the
  provider's response and are not personal. The telemetry privacy test is
  extended to an activity-recording job.
- **Retention and purge.** Sessions and their steps are deleted
  `CAMPBOOKS_ACTIVITY__RETENTION_DAYS` (default 30, 1 to 365) after they
  start. `workspaces.workspace_deleting` purges the workspace's rows after
  the grace period; `workspaces.member_left` deletes the member's private
  sessions. The `activity.session_changed` events carry ids only and fall
  under #65's event retention. Reasoning blobs (later) are detached on
  prune and purge and swept by `kernel.sweep_blobs`.

## Decision 5: the page

### Where it lives

| | **Sidebar item "Activity" with a live Visor** | A section of Settings | A drawer opened from the Visor on any page |
| --- | --- | --- | --- |
| Reachable while working | G then A from anywhere; one click | buried; reads as an audit log | yes, but a drawer cannot hold a list and a timeline at 375px |
| "What is it doing now?" at a glance | the sidebar Visor animates while a session runs | no | only when open |

**Chosen (owner approved): the sidebar item**, with the Visor as its icon (`thinking` while a
visible session runs, `alert` while one failed in the last hour, otherwise
`idle`; `asleep` when no AI role is configured). On phones the tab bar keeps
Home, Inbox, Settings, and the Visor sits in the top bar as a button to
Activity, so the live state is always on screen. The reader's "Why?" gains
"See what Campbooks did" linking to the session about that message.

### Screens

- **List** (`/w/$workspaceId/activity`): Segmented All / Running / Needs a
  look (unsure answers, failures, stalls). Each row: the session's Visor,
  its line ("Sorting 12 new messages in Work", "Sorted 12 new messages in
  Work: 3 suggestions"), time and duration, models used. Running rows show
  progress ("7 of 12"). J/K move, Enter opens. Pages of 30, infinite scroll.
- **Session** (`…/activity/$sessionId`, beside the list from 64rem, its own
  screen below): a header (kind, status with an `aria-live="polite"`
  announcement on change, started, duration, roles and models, tokens), then
  the timeline: one group per subject, newest last while running, each
  with the message's subject and sender (from `conversations`) and its
  stages: "Rules: suggests Newsletters, 95% sure", "Model: Primary, 97%
  sure. Not applied: answers are suggestions for now", "Waiting its turn",
  "The model could not be reached". Each group: Why? (a Disclosure with the
  reasons and evidence, translated by rule id with the conversations
  module's strings), Open conversation (O). Esc goes back.
- **Empty** states: no AI configured (Visor asleep, how to configure the
  `decide` role), nothing yet (Visor idle, "When mail arrives, you will see
  Campbooks sort it here").

### The Visor carries the live status

| Session / step | Visor |
| --- | --- |
| step working on the rules stage (reading headers, the body) | `reading` |
| step working on the model stage | `thinking` |
| waiting its turn (stage pending) | `idle` |
| done, nothing applied, any answer below its threshold | `unsure` |
| done, something applied (a policy that lets rules sort) | `happy` |
| failed or stalled | `alert` (then `dizzy`) |
| no AI role configured | `asleep` |

**The new emotions** belong to the design system, not to this change
(owner approved). Only `dizzy` has a use here (our side failed); `starry` (a
big win) and heart eyes (warmth) have no producer in this change. They ship
with the reworked motion, thinking as option B (hopping eyes, legible at
22px), in the separate `visor-emotions` change, with stories, both themes,
reduced motion and the 22px legibility check. This page uses `alert` for
failures until `dizzy` exists, then switches with a one-line mapping.

### Keyboard, languages, sizes

- New shortcuts in the registry: `nav.activity` (G then A, global),
  `activity.open_conversation` (O, item); the list uses the existing
  `list.*` keys and Esc. Every action shows its keys. The conflict check
  must pass.
- New namespace `activity` in en, pt-PT, es-ES, fr (plurals with
  `_one`/`_many`/`_other`; "Campbooks" in copy, never "Scout"); sentence
  case, no em dashes.
- 375px: list and session are separate screens, the timeline stacks, Why?
  folds; desktop: list beside session. Light and dark; axe in stories.

## Decision 6: slicing

Each slice is one OpenSpec change and one PR that archives itself, with its
recording.

1. **`assistant-activity` (this change).** Kernel port, `activity` module,
   triage recording in `conversations`, `conversations.list_decisions`, the
   Activity page with live updates and the live Visor, ADR 0015, docs.
   Recording: 375px and desktop, a fake Gmail batch arriving and its
   session filling in live, then Why? and Open conversation.
   If the diff grows past what one review can hold, it splits at the API
   (backend with a terminal recording first, then the page) without changing
   this design.
2. **`decision-corrections`.** "This was wrong" on every triage answer, on
   this page and in the reader: `conversations.correct` and its undo for
   needs a reply, asks you to do something, has a deadline, urgency and
   suspicious (the stream stays a move); the timeline shows the correction.
   Browser recording.
3. **`visor-emotions`** (`lib/ui`, built in parallel by another agent):
   dizzy, starry, heart eyes and the motion rework with thinking option B.
   Storybook recording; then this page maps failures to `dizzy`.
4. **`assistant-activity-reasoning`** (with #47 or the first `complete`
   producer): `complete`/`vision` steps with token counts, reasoning blobs,
   the producer's live token stream, and the workspace switch that turns
   storing reasoning off (on by default).

Other modules record their own sessions in the changes that build them.

## AI use

No new AI call. The roles recorded are `decide` (and later `complete`,
`vision`). When the `decide` role is not configured, triage sessions have
only the rules stage, no pending model stage, and the page says the model
pass is off. Nothing on the page changes user data in this slice; corrections
(slice 2) are operations with undo.

## Risks and trade-offs

- **Event log volume.** Two events per message (working, done) plus one per
  batch, retained until #65 prunes the log. They are small and id-only; if
  volume matters before #65, the kernel can gain a change-only announcement
  (a feed change without an event). Rejected for now because it breaks the
  feed's rule that a change exists exactly when its event does.
- **A synchronous port into another schema in the producer's transaction.**
  A slow or failing activity write fails the producer's job, which retries.
  The writes are a few inserts and one update; the port has its own tests
  and the producer's tests run with the discarding default.
- **Joining by subject** can attach a model step to the wrong session if the
  same message were a subject of two triage sessions (it cannot: a message
  is stored once and gets one rules stage). A replay that finds no pending
  stage records nothing.
- **Session row contention**: concurrent model steps update their session's
  counters; one short row lock per step.
- **Two hops to render a message** (activity, then conversations). The
  conversations call is batched per visible page of subjects and cached per
  message.

## Rollout

- Migrations: `<ts>_activity__init.sql`. No kernel migration.
- Configuration: `CAMPBOOKS_ACTIVITY__RETENTION_DAYS`,
  `CAMPBOOKS_ACTIVITY__STALL_MINUTES` in `deploy/.env.example`; CHANGELOG.
- Web sessions signed in before the upgrade lack `activity:read` until they
  sign in again; the page shows "Sign in again to see activity" on 403
  instead of an error.
- No backfill: sessions start with the first batch after the upgrade.
