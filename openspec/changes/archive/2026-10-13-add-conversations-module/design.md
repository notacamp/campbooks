# Design

## Context

The foundations this builds on are merged: the durable event bus and job
queue (ADR 0012), content-addressed blob storage (`state.blobs`), the
operation catalogue (ADR 0013), tenancy with row-level security
(`Workspace<S>`, `begin_for`), and the typed `decide` role over Laya
(`campbooks-ai`, ADR 0009). The `connectors` module (#36), which will read
providers and produce change streams, does not exist yet; this module must be
testable without it, against recorded change batches (ADR 0010).

Laya's zero-shot answers proved confidently wrong in the first integration
(ADR 0009 note). Until #59 gives a question an eval set, answers are stored as
suggestions and never change data.

## Data ownership and events

Everything lives in schema `conversations`, owned by this module. Every table
has `workspace_id`, `ENABLE` + `FORCE ROW LEVEL SECURITY` and the policy
`workspace_id = nullif(current_setting('campbooks.workspace_id', true), '')::uuid`;
the migration grants `campbooks_app` DML on them. Ids are UUIDv7 from Rust.
Foreign keys stay inside the schema; other modules' records (users) are
referenced by id.

| Table | Holds |
| --- | --- |
| `inboxes` | id, channel (`email`), address (lower-cased), display name, `connection_id` (an id owned by connectors, nullable), created_at. Unique `(workspace_id, channel, address)`. |
| `inbox_members` | inbox_id, user_id, role `reader`/`sender`/`manager`. Who sees an inbox. |
| `conversations` | id, inbox_id, thread_key (nullable, unique per inbox), subject (of the first message), stream, stream_source (`default`/`rules`/`model`/`member`), last_message_at, message_count, unread_count, starred, archived_at (set while no message is in the inbox), trashed (every message in trash or spam), created_at, updated_at. |
| `messages` | id, inbox_id, conversation_id, provider_message_id, direction `inbound`/`outbound`, rfc_message_id, in_reply_to, reference_ids (text[]), sender (jsonb `{name, address}`), sender_address (lower-cased), recipients (jsonb `{to, cc, reply_to}`), subject, snippet (≤ 200 characters of text), sent_at, headers (jsonb, the subset below), categories, provider_labels, mailbox state (`in_inbox`, `in_spam`, `in_trash`), read, starred, bodies (jsonb `{raw?, text?, html?}` blob refs), participants (blob ref of the participants document), created_at, updated_at. Unique `(inbox_id, provider_message_id)`. |
| `attachments` | id, message_id, position, filename, content_type, size, inline, blob ref. |
| `labels` | id, inbox_id, name, provider_key (nullable, for provider-mirrored labels). Unique per inbox by name and by provider key. |
| `conversation_labels` | conversation_id, label_id. |
| `decisions` | id, message_id, conversation_id, question, source (`rules`/`model`/`knn`/`cascade`), reasons (jsonb `[{rule, evidence}]`), value (jsonb), options (jsonb, ordered labels), probabilities (jsonb), confidence, model, model_version, latency_ms, `acted` (bool), state_version, created_at. |
| `corrections` | id, conversation_id, message_id (latest inbound), question (`stream`), from_value, to_value, stream_source it overrode, decision_ids (the answers on record at the time, rules and model), corrected_by (user id), created_at. |

Indexes for the list queries:
`conversations (inbox_id, stream, last_message_at DESC, id DESC) WHERE archived_at IS NULL AND NOT trashed`
(the inbox view by stream) and `(inbox_id, last_message_at DESC, id DESC) WHERE NOT trashed`;
for threading `messages (inbox_id, rfc_message_id)`, `(inbox_id, in_reply_to)` and a GIN index on
`reference_ids`.

### Blobs

The batch's blob refs (raw MIME, text part, HTML part, attachments) were
`put` by the connector. `apply_changes` `attach`es each to its owner
(`conversations.message` or `conversations.attachment`) in the transaction
that writes the row, and `detach`es them when a message is removed. It also
writes each new message's **participants document**
(`ParticipantsDocument` v1: from, to, cc, reply-to and subject; content type
`application/vnd.campbooks.message-participants+json; version=1`) and attaches
it to the message. The batch document itself is never attached here (its
producer holds it).

### Events published (all workspace-scoped, v1)

**No personal data in events.** The event log is retained, so payloads
carry ids and non-personal facts only. Addresses, names and subjects are in
the participants document, a blob the event points at and that is deleted
with the workspace (blob purge). Consumers that need them (parties, search)
read the document, or keep their own projection built from it.

- `conversations.message_received` — an inbound message was stored for the
  first time: workspace, inbox, conversation, message, provider message id,
  channel, stream, sent_at, the participants document's blob ref, the text
  part's blob ref, attachment count.
- `conversations.message_sent` — an outbound message appeared (from the
  provider's own client today; from Campbooks once sending lands): same shape.
- `conversations.conversation_changed` — a conversation's state changed:
  `changes` (`read`, `starred`, `archived`, `stream`, `labels`, `messages`,
  `removed`), `origin` (`provider` for a change batch, `member` for an
  operation), the provider ids of the messages concerned, and the state
  afterwards (stream, unread count, starred, archived, trashed; absent once
  removed). For projections.
- `conversations.changes_requested` — the **outbound intent**: a member's
  operation the provider should mirror, as `items` of
  `{provider_message_id, change}` with change kinds `mark_read`,
  `mark_unread`, `star`, `unstar`, `archive`, `unarchive`,
  `add_label`/`remove_label` (label id and provider key when the provider has
  the label) and `move_to_stream`; `origin: user`. Changes from the provider
  never produce it, so nothing echoes back. Contract:
  `contracts/conversations.changes_requested.v1.schema.json`; the connectors
  module consumes it.

### Events consumed

- `conversations.decide_message` ← `conversations.message_received`
  (queue `ai`): the model pass.
- `conversations.schedule_workspace_purge` ← `workspaces.workspace_deleting`
  (mirror type, same name and version, checked by `cargo xtask arch`):
  enqueues `conversations.purge_workspace` at `purge_after`, which deletes the
  workspace's rows (its blobs are purged by the kernel through the workspaces
  module's purge).
- `conversations.create_inbox` ← `connectors.capability_granted` (mirror
  type): for `messages`, creates the inbox the connector minted (address and
  display name from the connection-account v1 blob it points at, the
  connection's id) and makes `granted_by` its `manager`; idempotent.
- `conversations.ingest_messages` ← `connectors.messages_changed` (mirror
  type): enqueues `apply_changes` for the batch, ordered per inbox, with the
  batch blob held until applied.

## The ingest contract

```text
MessageChangeBatch v1  (JSON document, stored as a blob, content type
                        application/vnd.campbooks.message-changes+json; version=1)
  inbox: InboxId
  changes: [
    { op: "upsert", message: {
        provider_message_id, thread_key?, rfc_message_id?, in_reply_to?,
        references[], from, to[], cc[], reply_to[], subject, sent_at,
        direction,                        // the connector knows Sent vs Inbox
        headers: { list_id?, list_unsubscribe?, precedence?, auto_submitted? },
        raw: BlobRef?, text: BlobRef?, html: BlobRef?,
        attachments: [{ filename, content_type, size, blob: BlobRef, inline }],
        mailbox: { in_inbox, in_spam, in_trash },
        labels: [{ key, name }],          // provider user labels, normalised
        categories: [ "updates" | "promotions" | "social" | "forums" ],
        read, starred } }
    { op: "remove", provider_message_id }
  ]
```

- The connector normalises provider vocabulary (Gmail `CATEGORY_UPDATES`,
  `UNREAD`, folders) into these fields; this module never sees provider
  strings.
- Job `conversations.apply_changes { inbox, batch: BlobRef }`, queue `sync`,
  enqueued with `ordering_key = "conversations.inbox:<id>"` so one inbox's
  batches apply strictly in order, and `unique_key = <inbox>:<batch hash>` so
  a re-sent batch is a no-op while pending.
- Application is idempotent per message: an upsert of a known
  `(inbox_id, provider_message_id)` updates flags, mailbox state and labels
  and publishes `conversation_changed` only if something changed; an unknown
  one inserts the message, threads it, and publishes `message_received` or
  `message_sent`. A `remove` deletes the row, detaches its blobs, recounts its
  conversation and deletes the conversation when it is empty.
- The contract is `contracts/message-change-batch.v1.schema.json` (JSON Schema
  2020-12); modules never import each other, so the connectors module
  validates what it produces against the same file. This module's Rust types
  (`campbooks_conversations::ingest`) read it, and a contract test validates
  every recorded fixture batch against the schema.
- The batch blob is attached to a `conversations.change_batch` record (owner
  id: the inbox) when it is enqueued and detached once applied, so the blob
  sweep cannot remove it while its job waits or retries; a batch whose job
  failed for good stays held for inspection until the workspace is purged.
- Job errors and logs never carry content: JSON decoding errors keep only
  their class and position, and provider errors only their kind.
- A batch for an inbox that does not exist yet is **retried**, not failed:
  the connector announces the inbox (`connectors.capability_granted`) and its
  first batch independently. A batch of another version, or naming another
  inbox than its job, fails for good.

## Threading

Per inbox, in order:

1. The connector's `thread_key` (Gmail `threadId`, Graph `conversationId`)
   when given: join the conversation with that key in this inbox.
2. Otherwise, RFC 5322: join the conversation of any message in this inbox
   whose `rfc_message_id` is in the new message's `In-Reply-To` or
   `References`.
3. Otherwise, a new conversation. Children that arrived before their parent
   (same inbox, referencing its `rfc_message_id`, no `thread_key`) are moved
   into it.

No subject-based threading. Conversations never span inboxes: access is per
inbox, and two members' copies of one thread are two conversations (a
projection can relate them later).

## Streams and the decide pass

### One source of rules: `campbooks_ai::decide`

This module writes **no header or keyword rules of its own**. The
deterministic signals (List-Unsubscribe / List-Id / Precedence, MIME and
attachment types, no-reply senders, per-language keyword and date cues,
look-alike domains) live in one place, `campbooks_ai::decide::signals::email`,
behind the `decide::strategy` trait (rules, k-NN, model, cascade), built and
measured by #59. Its eval measures exactly the code that sorts mail here.

The module builds the shared **`Envelope`**
(`campbooks_ai::decide::signals::email::Envelope`) from the stored message:
sender, recipients, subject, the list and automation headers and `Reply-To`,
attachment names and types, and the start of the text body (16 KiB read; the
model reads its first 1,500 characters). It asks the triage questions
(`signals::email::triage_questions`, worded and labelled by #59) of two
strategies (`campbooks_ai::decide::strategy::Strategy`):

- **rules**: `EmailSignals`, when a message is stored (no I/O beyond the text
  blob), so its stream is right when it first appears;
- **model**: `strategy::Model` over the `decide` role, in
  `conversations.decide_message`, when the role is configured.

Each returns, per question, an answer (`Decided`: the `Answer` with its
probabilities and confidence) and its `Reason`s (stable rule id + evidence,
for the client's "Why?"), or abstains. Every answer is stored in `decisions`
with its source, strategy name, value, probabilities, confidence, model,
reasons and the envelope version. k-NN over the workspace's corrections and
cascades come in when the policy has a measured reason to use them.

### Questions and the per-question policy

The policy (`domain::policy::Policy`, injected through `ConversationsDeps`)
says, per question and source, whether an answer is only **recorded** or may
**act** from a confidence (`answer.accept_if`). `Policy::baseline()` is the
table measured by `cargo xtask eval decide` on mail-triage v1
(`crates/ai/evals/decide/mail-triage/reports/baseline.json`, #59): rules act
only where the operating point found on `dev` holds the question's target
precision on `test`, from that point; model answers have no measured
operating point yet and never act.

| Question | Type | Target | Rules (operating point, holds?) | Model |
| --- | --- | --- | --- | --- |
| `stream` | choice: primary, updates, receipts, newsletters | 95% | record (0.70: 87%, no) | record |
| `needs_reply` | noul | 90% | act ≥ 0.88 (94%, yes) | record |
| `asks_action` | noul | 90% | act ≥ 0.75 (95%, yes) | record |
| `has_deadline` | noul | 90% | act ≥ 0.75 (90%, yes) | record |
| `urgency` | score 0 to 2 | 85% | act ≥ 0.75 (93%, yes) | record |
| `suspicious` | noul | 98% | record (0.93: 97%, no) | record |

So, shipped as is, **no conversation is sorted out of Primary**: the stream
answers are kept as suggestions (with their reasons) until a strategy
holds its target; the placement code runs as soon as the table lets it (the
tests enable it with a test policy). The other questions' accepted answers
are marked `acted`, for the consumers that will use them (tasks, attention).
Changing the table is a reviewed code change citing a newer report, not an
operator setting.

A stream is assigned only from an accepted answer; with none, the
conversation goes to Primary (`stream_source = default`). The stream is set
from the conversation's first inbound message; later messages never re-sort
it, and a member's move sticks (`stream_source = member`);
`stream_source = rules` marks a rules placement, and the answer's
reasons in `decisions` name the signal. Question ids match #59's eval
set (`stream`, `needs_reply`, `asks_action`, `has_deadline`, `urgency`,
`suspicious`).

The model pass (purpose `conversations.triage`, role `decide`, all
questions in one call) runs as `conversations.decide_message` on
`message_received`, only for inbound messages sent within the backfill window
(`CAMPBOOKS_CONVERSATIONS__DECIDE_BACKFILL_DAYS`, default 14; a first sync's
old mail gets no model call, per "cost follows activity"), only when the
`decide` role is configured, and once per message (a replayed delivery finds
the answers stored and asks nothing). Provider errors are retried with only
their kind in `last_error` (a provider may echo the request). The model call happens before the job
opens its transaction; answers are written in it. A provider failure retries
the job; a validation error is fatal.

**AI summary (config rule):** role `decide`; fallback when not configured:
rules only, no model rows; undo: no model answer is applied in this change,
no stream is placed under the baseline policy, and any placement is undone by
a member's move.

### Corrections

`conversations.move_to_stream` sets the stream, records a `corrections` row
(from, to, the source it overrode, the `stream` decisions on record, the
member) and publishes `conversation_changed`. Moving back is the same
operation (its own undo). Corrections are the labelled examples #59 reads;
the input is rebuilt from the message with the recorded `state_version`, so
content is not duplicated.

## Access

`Workspace<S>` resolves the workspace (404 otherwise). Within it, a user sees
the inboxes they have an `inbox_members` row for; a client token (no user)
sees every inbox of its bound workspace. A conversation or message in an
inbox the caller cannot see answers 404. Changing a conversation (read,
star, archive, labels, stream) takes the `sender` role or above: a
`reader` sees it but gets 403 `inbox_read_only`. A client token changes
through its `conversations:write` scope. Inbox-level access is checked in the
queries (jobs have no user); row-level security stays the workspace line.

## Endpoints and scopes

All under `/api/v1/w/{workspace_id}`.

| Operation | Endpoint | Scope | x-campbooks |
| --- | --- | --- | --- |
| `conversations.list_inboxes` | `GET /inboxes` | `conversations:read` | query, assistant |
| `conversations.list` | `GET /conversations?stream=&inbox=&cursor=&limit=` | `conversations:read` | query, assistant |
| `conversations.get` | `GET /conversations/{id}` (messages with plain text) | `conversations:read` | query, assistant |
| `conversations.get_message_body` | `GET /conversations/{id}/messages/{message_id}/body` (JSON `{text, html}`) | `conversations:read` | query |
| `conversations.mark_read` / `conversations.mark_unread` | `POST /conversations/{id}/read`, `/unread` | `conversations:write` | low, undo each other, automation, assistant |
| `conversations.star` / `conversations.unstar` | `POST …/star`, `…/unstar` | `conversations:write` | same |
| `conversations.archive` / `conversations.unarchive` | `POST …/archive`, `…/unarchive` | `conversations:write` | same |
| `conversations.move_to_stream` | `POST …/stream` `{stream}` | `conversations:write` | low, automation, assistant |
| `conversations.add_label` / `conversations.remove_label` | `POST …/labels` `{name}`, `DELETE …/labels/{label_id}` | `conversations:write` | low, undo each other |

The list is ordered by `last_message_at DESC, id DESC`; the cursor is that
pair, opaque (base64url). Errors the module raises carry a stable `code` and a
message from its catalogue (`crates/modules/conversations/i18n/`:
`invalid_limit`, `invalid_cursor`, `invalid_label_name`), rendered in the
caller's locale like every problem+json. HTML is never served as `text/html`: it is a JSON
string the client renders only in its sandboxed iframe (`lib/ui/safe-html`).

## Change feed

`message_received` / `message_sent` declare their conversation and message
as upserted, and `conversation_changed` its conversation (deleted once
gone), each **within its inbox** (`conversations.inbox`), so the feed shows
them only to members who may see that inbox. The module implements the
kernel's `Visibility` port (`conversations::visibility()`, registered by the
server): a user sees an inbox's changes when they are its member, a client
token bound to the workspace sees every inbox, and a container of another
module's type is not this module's to grant. Inbox creation declares no
change yet (it has no event of its own); listing inboxes is a refetch.

## Composition: constructor injection

The module is composed with its dependencies: the server's `Composition` (shared with the connectors module) reads every
module's dependencies once: for this module a `campbooks_ai::Gateway` when
any `CAMPBOOKS_AI__*` role is set, and `ConversationsConfig` from
`CAMPBOOKS_CONVERSATIONS__*`; it calls
`campbooks_conversations::module(ConversationsDeps { ai, config, rules, policy })`.
Describing the modules (OpenAPI, scopes, events catalogue, `cargo xtask`)
uses the inert `Composition::default()`. Nothing is looked up at run time.

Jobs and event handlers were stateless types (`run(self, ctx)`), so they
could not hold what the module was composed with. The kernel gains the
smallest typed hook: `JobHandler` and `EventConsumer`, like `Job` and
`EventHandler` but run through `&self`, registered as values with
`Background::handler(h)` and `Background::consumer(c)` (enqueued with
`jobs::enqueue_to::<H>`). The registry's run functions became shared closures
over those values. `apply_changes` owns the rules strategy;
`decide_message` owns the model strategy and the backfill window. Routes
need no dependencies in this change; when one does, it captures them in its
router the same way.

## Alternatives considered

**Threading.** (a) Provider thread id only: matches what the member sees in
Gmail/Outlook, but IMAP has none and every IMAP message would be its own
conversation. (b) RFC 5322 `References`/`In-Reply-To` only: provider-neutral,
but disagrees with Gmail's own grouping (Gmail also groups by subject) so
"archive this thread" would touch a different set of messages than the
provider does. (c) **Hybrid, provider key first (chosen):** agrees with the
provider where one exists, still threads IMAP. Costs the out-of-order
reparenting step.

**One table per channel vs a channel column.** Per-channel tables (`email_messages`,
later `chat_messages`) give tight typed columns but every list, label, stream
and decision query must union them. **A channel column on `inboxes`
(chosen)**, common typed columns on `messages`, and email specifics in a small
`headers` jsonb keeps one list query; a second channel is additive.

**Bodies in blobs only vs extracted text in Postgres.** Full text in Postgres
makes reads one query but bloats rows, backups and the buffer cache with
content most lists never show (ADR 0011). **Blobs for raw/text/HTML plus a
200-character snippet in Postgres (chosen)**: lists stay one query; reading a
conversation fetches a bounded number of text blobs. Search will keep its
own extracted text and vectors.

**Where deterministic rules live.** (a) In the connector (provider
categories decide the stream): provider-specific, and IMAP has none. (b) As
data in a rules table: that is the automation module (M2). (c) Named rules in
this module: simple, but a second copy of the signals #59 is evaluating, so
the eval would measure code that does not sort the mail. (d) **One home in
`campbooks_ai::decide::signals::email` behind the `decide::strategy` trait
(chosen)**: rules and model answers share one shape (answer + reasons),
the eval measures the code in production, and switching a question from rules
to a cascade is a policy change here, not new code. Workspace rules arrive
with automation and run after these.

**How the batch reaches the module.** (a) The batch inline in a job or event
payload: simplest, but subjects and addresses of every message would sit in
`kernel.jobs` and, for an event, forever in `kernel.events`, against the
"payloads hold ids" rule. (b) A connectors→conversations crate dependency
(`architecture.toml` exception) so the connector calls `apply` directly:
couples every capability's adapter to its domain crate. (c) **The batch as a
blob, a job holding `{inbox, batch}` (chosen)**, triggered by a
`connectors.messages_changed` event with the same ids: id-only payloads,
recorded batches are fixture files, and re-applying is re-enqueueing. The
shape is a JSON Schema in `contracts/`, since neither module can read the
other's types.

**Reaching the AI gateway from a job.** (a) A typed service map on
`AppState` (`state.service::<Gateway>()`): small, but a service locator,
with runtime lookups and dependencies hidden from the module's signature.
Rejected at the gate. (b) A process-wide `OnceLock` per module: breaks
parallel tests with different fakes. (c) A kernel `Decide` port: re-declares
`campbooks-ai`'s types in the kernel. (d) **Constructor injection (chosen):**
the server composes `module(deps)`, and handlers that need dependencies are
values that own them (`JobHandler`, `EventConsumer`).

**Personal data for consumers.** (a) Participants and subject in event
payloads: simplest for parties and search, but the event log is never pruned
and outlives a deleted workspace. (b) **A versioned participants document as
a blob, pointed at by the event (chosen):** deleted with the workspace;
consumers read it or project from it.

**Outbound intent.** (a) The connector reads `conversation_changed` and
works out what to do: it would need this module's state to know which
messages and labels. (b) **An explicit `changes_requested` event (chosen)**,
message by message, with change kinds and ids only, published only for
member operations.

**Copied from the Rails app on `main`?** Only the unique
`(account, provider_thread_id)` grouping, on the merits (the bulk-header
signals it used now live in #59's shared signals, not here). Not copied: per-provider sync inside the domain, inbox groups as
rules tables, and storing bodies in Postgres.

## Risks / Trade-offs

- **The event log is never pruned.** Events here carry no personal data, so
  nothing personal outlives a deleted workspace through them; event-log
  retention and per-workspace purge are a kernel follow-up the lead tracks.
- **Rules misfile** (a personal email from a list, an invoice without an
  attachment). Contained: a move is one operation, it sticks, and it is
  recorded as a correction for #59.
- **Reparenting late parents** touches several rows; bounded to the inbox and
  serialised by the per-inbox ordering key.
- **Large backfills**: one job per batch, ordered per inbox; fairness across
  workspaces is the queue's. The AI pass is skipped for old mail.
- **Unsanitised HTML** reaches the client as data; the sandboxed iframe with a
  strict CSP is the boundary (AGENTS.md), never the API.
