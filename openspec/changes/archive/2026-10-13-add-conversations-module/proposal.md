# Proposal

## Why

Milestone M1 is "an inbox that sorts itself" (`docs/domain-map.md` →
*conversations*). Every later piece of it needs messages to exist as typed
records first: `parties` builds people and services from `MessageReceived`,
`attention` raises "reply" items from conversations, `search` indexes them,
and the client's Inbox lists them by stream. Today nothing in Campbooks Next
can hold a message.

This change is the first slice of the `conversations` module: the ingest
contract the `connectors` module (#36) will produce, idempotent application
of message changes into inboxes, conversations and streams, the per-message
`decide` pass recorded as **suggestions** (ADR 0009 note: zero-shot answers
are not reliable until a question has an eval set, #59), and the read/write
operations a client needs to show and tidy an inbox.

Tracking issue: https://github.com/notacamp/campbooks-cloud/issues/37.

## What Changes

- **New module `conversations`** (`crates/modules/conversations`, schema
  `conversations`), scaffolded by `cargo xtask new-module conversations`.
  Language: Channel, Inbox, Conversation, Message, Stream, Label (the human
  stream is **Primary**).
- **Ingest contract, no provider code.** A public, versioned
  `MessageChangeBatch` (v1): for one inbox, an ordered list of upserts and
  removals of normalised messages (provider message id, thread key,
  RFC 5322 ids, a headers subset, from/to/cc/reply-to, subject, sent_at, body
  blob refs for the raw MIME and the text/HTML parts, attachments, mailbox
  state, provider labels, read/starred flags). The batch document is itself
  a blob; the job `conversations.apply_changes { inbox, batch }` carries only
  ids. It applies idempotently (upsert by `(inbox_id, provider_message_id)`),
  in order per inbox, threads messages into conversations, attaches blobs and
  publishes events in the same transaction.
- **Events** (v1, ids and non-personal facts only; addresses and subjects are
  in a per-message participants document blob the events point at):
  `conversations.message_received`, `conversations.message_sent`,
  `conversations.conversation_changed`, and `conversations.changes_requested`
  (a member's change the provider should mirror).
- **Streams:** the built-in Primary, Updates, Receipts and Newsletters.
  A conversation's stream is assigned only from an **accepted answer** to the
  `stream` question, else Primary. The shipped policy comes from #59's eval
  baseline, where no strategy holds the stream target yet, so stream answers
  are suggestions for now. This module writes no header or keyword
  rules: it builds a compact `Envelope` from the stored message and asks the
  shared strategy in `campbooks_ai::decide` (signals and strategies owned and
  evaluated by #59). Which answers may act is a per-question policy table.
- **The decide pass, as suggestions:** `conversations.decide_message`
  consumes `message_received` and asks six typed questions (stream, needs
  reply, asks for something, has a deadline, urgency, suspicious) through
  `campbooks-ai` in one call. Every answer, from rules or the model, is
  stored in `decisions` with its source, reasons, probabilities,
  confidence and model. A per-question switch says which sources may act;
  model answers are recorded only, so #59 switches a question by changing
  its policy.
- **Corrections:** moving a conversation to another stream records a
  `corrections` row (the labelled example #59 needs), linked to the rule and
  the AI suggestion it overrode.
- **Operations** under `/api/v1/w/{workspace_id}/…` with `operationId`s and
  `x-campbooks`: list inboxes; list conversations by stream (cursor
  pagination); get a conversation with its messages (plain text from blobs);
  get one message's body (text and HTML as JSON, for the client's sandboxed
  iframe); mark read / unread, star / unstar, archive / unarchive (undo
  pairs); move to a stream; add / remove a label.
- **Scopes:** `conversations:read`, `conversations:write`.
- **Composition.** The module is composed with its dependencies (the AI
  gateway, settings, rules strategy, policy) by the server's `Composition`,
  and hands them to its jobs through the kernel's `JobHandler` /
  `EventConsumer` (handlers registered as values; the same kernel change
  landed with the connectors module, #36).
- **Contracts** in `contracts/` (JSON Schema): `message-change-batch.v1`,
  `connectors.messages_changed.v1` (the trigger connectors will publish) and
  `conversations.changes_requested.v1` (the outbound intent connectors will
  consume).
- **Setting** `CAMPBOOKS_CONVERSATIONS__DECIDE_BACKFILL_DAYS` (default 14): mail
  sent earlier gets no model pass.
- **Connectors wiring:** consumes `connectors.capability_granted` (creates
  the inbox the connector minted, the granting member as `manager`, address
  and name from the connection-account blob) and `connectors.messages_changed`
  (enqueues `apply_changes` for the announced batch, ordered per inbox).
- **Change feed:** conversation and message changes are declared within
  their inbox, and the module implements the change feed's `Visibility` port
  (inbox membership; a client token sees every inbox).
- **Workspace deletion:** consumes `workspaces.workspace_deleting` and purges
  the module's rows at the end of the grace period.
- **Dev/test helper** to create an inbox and load the recorded fixture
  batches (fictional mail in en, pt-PT, es-ES and fr).

## Capabilities

### New Capabilities

- `conversations`: inboxes, conversations and messages on any channel (email
  first), ingest of normalised message changes, streams, labels, read/star/
  archive state, the decide pass recorded as suggestions, and corrections.

### Modified Capabilities

None.

## Impact

- Adds a module approved by `docs/domain-map.md` (M1, *conversations*) and
  ADRs 0009–0012. No cross-module crate dependency, so no
  `architecture.toml` entry; it consumes `workspaces.workspace_deleting`
  through its own mirror type, checked by `cargo xtask arch`.
- Kernel: no change of its own (the `JobHandler` / `EventConsumer` hook it
  uses landed with the connectors module). Server: `Composition` gains the
  module's dependencies, and the module's `Visibility` port is registered.
- Self-hosters: nothing to do; one optional setting
  (`CAMPBOOKS_CONVERSATIONS__DECIDE_BACKFILL_DAYS`). The model pass runs only when
  `CAMPBOOKS_AI__DECIDE__*` is configured; without it messages are sorted by
  the shared rules strategy alone.

## Non-goals

- Provider sync, cursors, push or polling, and carrying changes out to the
  provider (`connectors`, #36, which consumes `changes_requested`).
- Mounting the routes under the workspace route of #451: whichever of #451
  and this change lands second does it.
- Sending, drafts, scheduled send, signatures (a separate change).
- Custom streams created by the workspace (the table shape allows them; the
  operations come later).
- Inbox sharing operations (rows are created with the connecting member as
  `manager`; sharing comes with connectors).
- `MessageDecided` and `AttachmentReceived` events, parties and attention
  projections, search indexing, UI (the client module stays the scaffold's
  placeholder).
- Acting on any model answer.
- Writing sorting rules here (they live in `campbooks_ai::decide`, #59).
