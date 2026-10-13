# Design

## Context

`conversations` owns inboxes (with `reader` / `sender` / `manager` roles),
conversations and messages; `connectors` owns connections, credentials and
the Gmail adapter, and turns Gmail into `MessageChangeBatch` documents that
`conversations` applies (ADR 0010). Modules never import each other: they
talk through events published in the transaction of the change (ADR 0012)
and through blob documents whose shape lives in `contracts/`. Sending is
the first **outbound change** that carries content, so it needs a document,
not just ids.

Constraints that shape the design:

- A message must **never go out twice**, even when a worker dies after
  Gmail accepted it, or a request to Gmail times out.
- The member gets a few seconds to **take a send back**.
- **No personal data in events or logs**: addresses, subjects and bodies
  travel only in blobs.
- The sent message must show in its conversation **as soon as it is sent**,
  and the next sync must not make a second copy.
- Self-hosting stays one container plus Postgres: no new service.

## Data ownership

| Data | Owner | Where |
| --- | --- | --- |
| Drafts (recipients, subject, body, quoted history, forwarded attachments, send state) | `conversations` | `conversations.drafts` |
| The outgoing message document (what to send) | `conversations` writes, `connectors` reads | blob, `contracts/outgoing-message.v1.schema.json` |
| A send's progress at the provider (attempts, provider id) | `connectors` | `connectors.sends` |
| The sent message | `conversations` (via the usual batch) | `conversations.messages` |

`conversations.drafts` (workspace-owned, forced RLS like every table):
`id`, `workspace_id`, `inbox_id`, `author` (a user id; `NULL` for a client
token), `kind` (`new`, `reply`, `reply_all`, `forward`), `conversation_id`
and `source_message_id` (the message replied to or forwarded; `NULL` for a
new message; `ON DELETE SET NULL`), `to_recipients` / `cc_recipients` /
`bcc_recipients` (jsonb `{name?, address}` lists), `subject`, `body` (what
the member writes), `quoted` (the quoted history, editable),
`in_reply_to`, `reference_ids`, `thread_key`, `attachments` (jsonb: the
forwarded attachments with their blob references, held by the draft),
`version` (bumped by every save), `state` (`draft`, `scheduled`,
`sending`, `failed`), `send_generation`, `send_at`, `send_id`,
`rfc_message_id`, `failure_reason`, `created_at`, `updated_at`.

Drafts are **private to their author**: another member of the inbox, even a
manager, gets 404. A client token sees the drafts client tokens created.

`connectors.sends`: `id` (= the send id, the idempotency key), `workspace_id`,
`stream_id`, `inbox_id`, `document` (the blob reference, held while the send
is open), `rfc_message_id`, `state` (`pending`, `sent`, `failed`),
`attempts`, `attempted_at`, `provider_message_id`, `thread_key`,
`failure_reason`, timestamps.

### Blobs

- The forwarded attachments' blobs are attached to the draft
  (`conversations.draft`) when it is created, so they outlive the original
  message being deleted; dropping one in the composer detaches it.
- The outgoing document is put by `conversations.release_send` before the
  transaction that publishes `send_requested`, attached to the draft there,
  and attached to the `connectors.sends` row by `connectors.queue_send`.
  Both release it when the send ends.

## How a send flows

```
composer ── POST /drafts/{id}/send ──► conversations: state=scheduled, send_at=now+undo
                                        enqueue conversations.release_send (run_at=send_at)
   ◄── 202 { state, send_at } ── (Undo: POST /drafts/{id}/cancel_send → state=draft)

conversations.release_send (at send_at; no-op unless still scheduled at that generation)
   put outgoing-message blob; state=sending, send_id, Message-ID
   publish conversations.send_requested { inbox_id, send_id, message: blob }

connectors.queue_send (consumer)
   insert connectors.sends (id = send_id, ON CONFLICT DO NOTHING), hold the blob
   enqueue connectors.send_message (unique per send)

connectors.send_message (job, up to 5 attempts)
   if state != pending → done
   if attempts > 0 → messages.list q=rfc822msgid:<id>  ── found → finish as sent
   record the attempt (own transaction, committed before the call)
   build RFC 5322 (mail-builder), POST upload/gmail/v1/users/me/messages/send
       (multipart: { threadId } + message/rfc822)
   ok → in the job's transaction: index the sent message, store and announce
        a MessageChangeBatch (connectors.messages_changed), state=sent,
        publish connectors.send_completed { outcome: sent, provider_message_id },
        make the stream due now
   definite refusal (4xx) → state=failed, send_completed { outcome: failed, reason }
   unclear (5xx, timeout) → retry no sooner than 30 s (the lookup runs first)

conversations.record_send (consumer of connectors.send_completed)
   sent   → delete the draft, release its blobs, publish conversations.draft_sent
   failed → state=failed, failure_reason, publish conversations.send_failed
```

### Idempotency

Three layers, each closing a different gap:

1. **The request.** `POST …/send` is a mutating operation, so the kernel's
   `Idempotency-Key` layer replays a retried request. And the state machine
   refuses a second send of a draft that is already `scheduled` or
   `sending` (409 `draft_not_editable`).
2. **The intent.** `send_id` is minted once per release and is the primary
   key of `connectors.sends`: a redelivered `send_requested` inserts nothing
   and enqueues nothing new (`unique_key`).
3. **The provider call.** Before each call the job commits `attempts + 1`
   and `unclear = true` in its own transaction: from then until Gmail gives
   a definite answer, the message may be out. Only an acceptance or a
   refusal of the send itself (any 4xx) clears it; a 5xx, a timeout, a
   worker that died, and a 2xx whose answer cannot be read all leave it
   set. While unclear, the next attempt first asks Gmail for a message with
   our `Message-ID` (`rfc822msgid:` search, spam and trash included): found
   means sent; not found clears the mark. No attempt calls Gmail sooner
   than the HTTP timeout plus 30 s after the previous one, so a runner that
   lost its lease cannot overlap a call still in flight, and Gmail's search
   index has caught up. Any failure while unclear (needs_reauth,
   not_connected, the lookup failing, a later 4xx before a proven miss) is
   `unknown_outcome`, never "not sent".
4. **Settling.** A send whose attempts run out, or that cannot go on
   (fatal), is settled in a transaction of its own: `unknown_outcome` if
   unclear, else `unavailable` (nothing was sent; the draft is editable
   again). Behind every send, `connectors.queue_send` enqueues a check with
   the same ordering key, 30 minutes on; it runs only once no earlier job
   for the send is alive, and settles a send whose job died the same way
   (after a lookup when unclear). On the conversations side,
   `conversations.check_send` fails a draft still "sending" an hour after
   release with `unknown_outcome`, and `record_send` still accepts a late
   outcome for it.

The `Message-ID` is `<{send_id}@{domain of the From address}>`, generated by
`conversations` at release; Gmail keeps a `Message-ID` the client sets. If
the last attempt still ends unclearly, the send fails with
`unknown_outcome`, and the UI says to check the Sent folder before sending
again (it may have gone). That is the one case where Campbooks cannot know.

### Undo send

`send_draft` names the draft `version` it sends (409 `draft_changed`
otherwise), so the server never sends an older save than the member saw;
the client sends only after its last save of what is on screen succeeded.
It sets `state = scheduled`, `send_generation + 1`,
`send_at = now + undo`, and enqueues `conversations.release_send
{ draft, generation }` with `run_at = send_at`. `cancel_send` puts a
scheduled draft back to `draft` (409 `too_late` once it is `sending`). The
release job locks the draft row and acts only if it is still `scheduled` at
its generation, so a cancel and a release cannot both win, and a stale job
from an earlier send of the same draft does nothing. The window is
`CAMPBOOKS_CONVERSATIONS__UNDO_SEND_SECONDS` (default 10; 0 to 60). The UI
counts down from the server's `send_at`.

### Replies and threading

`create_draft` with `kind` `reply`, `reply_all` or `forward` and a
`message_id` fills, on the server:

- **Recipients.** Reply: the message's `Reply-To`, else its sender (for a
  message the inbox sent: its `To`). Reply all: that, plus the message's
  `To` and `Cc`, without the inbox's own address and without duplicates
  (first occurrence kept). Forward: none.
- **Subject.** `Re: ` / `Fwd: ` added unless already there
  (case-insensitive, also `Fw:`).
- **Threading.** `In-Reply-To` = the message's `Message-ID`; `References` =
  its `References` plus its `Message-ID` (the last 20, RFC 5322 §3.6.4);
  `thread_key` = the conversation's provider thread (Gmail `threadId`). A
  forward keeps the thread key and references too, so Gmail files it in the
  same thread, like Gmail itself does.
- **Forwarded attachments** (forward only): every non-inline attachment of
  the message.

The **quoted history** is written by the client ("On Tue 14 Oct, Ana Lopes
wrote:" plus the text quoted with `> `; for a forward, the "Forwarded
message" header block) and saved in `quoted`, because the client knows the
member's language, time zone and date format; the server stores it as text
and appends it to the body when sending. An API caller may leave it empty.

### Body and formatting

The member writes plain text with a few marks: `**bold**`, `*italic*`,
`- ` bullets, `1. ` numbered items, `> ` quotes, and bare `https://` links.
At release, `conversations` renders the HTML alternative with its own small
renderer: every character is escaped first, then only those marks become
`<strong>`, `<em>`, `<ul>/<ol>`, `<blockquote>` and `<a href>` (http and
https only). There is no way to put raw HTML into a message. The plain-text
part is the text as written. The quoted history becomes a `<blockquote>`.

## The contracts

`contracts/outgoing-message.v1.schema.json` (media type
`application/vnd.campbooks.outgoing-message+json; version=1`), provider
neutral: `version`, `inbox`, `send_id`, `rfc_message_id`, `from`, `to`,
`cc`, `bcc`, `subject`, `date`, `in_reply_to?`, `references`,
`thread_key?`, `text`, `html?`, `attachments[{filename, content_type,
blob}]`. The bodies are inline (the document is itself a blob);
attachments are blob references in the workspace's store.

`contracts/conversations.send_requested.v1.schema.json`: `workspace`,
`inbox_id`, `draft`, `send_id`, `message` (blob reference).

`contracts/connectors.send_completed.v1.schema.json`: `workspace`,
`inbox_id`, `send_id`, `outcome` (`sent` / `failed`),
`provider_message_id?`, `reason?` (`not_connected`, `needs_reauth`,
`rejected`, `too_large`, `unavailable`: nothing was sent, `unknown_outcome`:
it may have been).

Both sides keep a contract test against each file.

## Events

| Event | Publisher | Consumer | Feed changes |
| --- | --- | --- | --- |
| `conversations.send_requested` v1 | `conversations` (release) | `connectors.queue_send` | draft upsert, in its inbox |
| `connectors.send_completed` v1 | `connectors` (the send job) | `conversations.record_send` | none |
| `conversations.draft_sent` v1 | `conversations` | none yet | draft delete |
| `conversations.send_failed` v1 | `conversations` | none yet | draft upsert |
| `connectors.messages_changed` v1 (existing) | `connectors` (the sent message) | `conversations` | as today |

Saving a draft publishes nothing: autosave would fill the retained event log
with noise, drafts are private, and the author's client reads its own
writes. Only send state transitions are events.

## Endpoints and scopes

New scope `conversations:send`: "Write drafts and send messages from your
inboxes." Every draft operation requires it; users also need the `sender`
or `manager` role on the draft's inbox (`reader` → 403
`inbox_read_only`; not a member → 404). The web client asks for every API
scope already, so it gets the new one without configuration.

| Operation | Method and path (under `/api/v1/w/{workspace_id}`) | Notes |
| --- | --- | --- |
| `conversations.list_drafts` | `GET /drafts?conversation_id=` | the caller's open drafts, newest first |
| `conversations.create_draft` | `POST /drafts` | `kind`, `inbox_id` (new) or `message_id` (reply/forward); 201 |
| `conversations.get_draft` | `GET /drafts/{draft_id}` | |
| `conversations.update_draft` | `PATCH /drafts/{draft_id}` | partial; `version` required, 409 `draft_changed` on mismatch; 409 `draft_not_editable` while scheduled or sending |
| `conversations.delete_draft` | `DELETE /drafts/{draft_id}` | discard; not while sending |
| `conversations.send_draft` | `POST /drafts/{draft_id}/send` (`{version}`) | 202; 409 `draft_changed`; `Risk::High`, assistant (with confirmation); undo `cancel_send`; 400 `no_recipients`, `invalid_address`, `attachments_too_large` |
| `conversations.cancel_send` | `POST /drafts/{draft_id}/cancel_send` | 409 `too_late` |

Limits: 100 recipients in all, 998 characters of subject, 1 MiB of body
plus quoted text, 25 MiB of forwarded attachments (Gmail's own limit).

## Configuration

`CAMPBOOKS_CONVERSATIONS__UNDO_SEND_SECONDS` (default 10, 0 to 60). Nothing
new for connectors: Gmail's `gmail.modify` scope, which the connection
already has, covers `messages.send`.

## Alternatives considered

**Where drafts live.**

- *(Chosen) In `conversations`.* Drafts belong to an inbox and a
  conversation, share its roles and its record references, and the domain
  map lists them there. No new schema owner, no new event plumbing for
  reading a conversation's context.
- *A new `compose` (or `outbox`) module.* Cleaner on paper, but it would
  have to learn inboxes, roles, conversations and messages through events
  and keep projections of all of them only to fill a reply, and every
  composer screen would read two modules. Rejected: machinery without a
  second consumer.
- *Gmail's Drafts as the source of truth.* Drafts would follow the member
  to Gmail's own apps, but every keystroke autosave becomes a provider call
  (quota, latency, offline), drafts become provider-specific (IMAP has no
  draft API), and the sync would have to stop skipping `DRAFT` messages and
  merge two writers. Rejected for now; mirroring drafts to the provider can
  be added later as an outbound change.

**Who builds the MIME message.**

- *(Chosen) `connectors`, from a provider-neutral document.* Microsoft
  Graph and Zoho send JSON, not MIME; keeping MIME in the Gmail adapter
  keeps `conversations` provider-free (ADR 0010).
- *`conversations` builds RFC 5322 and hands bytes over.* Simpler contract,
  but every non-MIME provider would have to parse it back.

**How the sent message reaches the conversation.**

- *(Chosen) The adapter announces what it sent as an ordinary batch,* built
  from the bytes it sent and indexed like a synced message. One write path
  for messages (threading, counts, events) and the sync recognises the
  message by its Gmail id.
- *`conversations` inserts a local copy on `send_completed`.* A second way
  of creating messages, which would duplicate threading and counting.
- *Wait for the next sync.* No new code, but the member waits up to a poll
  interval to see what they just sent.

**Undo send.**

- *(Chosen) A delayed release job in `conversations`.* Undo never crosses a
  module: before release nothing exists outside `conversations`.
- *Hold the send in `connectors` and cancel it there.* Needs a cancel event
  and races between cancel and send in another module.
- *Hold it in the client.* Lost when the tab closes; the assistant and the
  API would get no window.

**Idempotent sends.**

- *(Chosen) Record the attempt, look it up by `Message-ID` before a retry.*
  Gmail has no idempotency key for `messages.send`; the `Message-ID` is the
  only key we control that Gmail can search.
- *Send at most once (never retry an unclear attempt).* No duplicates, but
  every timeout loses the message silently.
- *Insert a Gmail draft, then send the draft.* `drafts.send` of an already
  sent draft fails, which detects duplicates, but it costs two calls and
  shows the message in the member's Gmail Drafts during the send.

**Copying from the Rails app on `main`.** The old app also kept drafts in
its own database (`DraftEmail`), which wins here on the merits above. It
sent synchronously inside the web request (`Emails::Sender`), with no undo
window and no protection against a timeout sending twice; that pattern is
not copied.

## Risks / Trade-offs

- **Gmail rewriting a `Message-ID`** would defeat the lookup. Gmail keeps a
  client-set one; this is checked on staging against a real test account
  (sending to that account only) before the production cutover, not in CI.
- **Search lag**: a message accepted seconds before a timeout might not be
  searchable yet; the 30 s wait before an unclear retry covers the common
  case, and `unknown_outcome` reports the rest honestly.
- **No live draft sync across devices** (no events on save). Opening the
  same draft on two devices ends in a 409 `draft_changed` on the older one,
  which reloads.
- **Quoted history written by clients** means each client must write it;
  the API documents the format, and an empty quote is valid.
- **Undo** is offered for the whole window the server gives (the toast
  lasts until `send_at`); an Undo that arrives after it says "Too late to
  undo: it was sent" rather than failing silently.
- **Shortcut change:** `c` (new message) is global, so the members page's
  "copy invitation link" moves from `C` to `Y`.
