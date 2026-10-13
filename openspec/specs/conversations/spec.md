# conversations Specification

## Purpose
Messages on any channel (email first): the inboxes a workspace receives and
sends on, the conversations and messages in them, the streams Campbooks sorts
conversations into, and the member's own labels and flags. Messages arrive as
normalised change batches from the connectors module; every inbound message
gets the decide pass, whose answers are recorded and act only where the
question's policy allows.

## Requirements

### Requirement: Conversations scopes
The `conversations` module SHALL declare the scopes `conversations:read` and
`conversations:write`, and every endpoint it adds SHALL require one of them.

#### Scenario: Anonymous caller
- **WHEN** a client calls a `conversations` endpoint without an access token
- **THEN** the response is 401 as `application/problem+json`

#### Scenario: Token without the scope
- **WHEN** a client with only `conversations:read` posts to `/api/v1/w/{workspace_id}/conversations/{conversation_id}/read`
- **THEN** the response is 403 as `application/problem+json`

### Requirement: Inbox access
Every table of the `conversations` schema SHALL be isolated per workspace by
forced row-level security. Within a workspace, a user SHALL see only the
inboxes they are a member of (`reader`, `sender` or `manager`), and a client
token bound to the workspace SHALL see every inbox. A conversation, message or
inbox the caller cannot see SHALL answer 404. Changing a conversation SHALL
take the `sender` role or above (or a client token); a `reader` SHALL get
403 `inbox_read_only`.

#### Scenario: A member of another inbox
- **WHEN** a member who is not in Sam's inbox reads one of its conversations
- **THEN** the response is 404

#### Scenario: A reader changes a conversation
- **WHEN** a member with the `reader` role on an inbox archives, marks read, labels or moves one of its conversations
- **THEN** the response is 403 with code `inbox_read_only` and nothing changes

#### Scenario: Another workspace
- **WHEN** a transaction set to one workspace counts the messages of another
- **THEN** it sees none

### Requirement: Applying change batches
The job `conversations.apply_changes` SHALL apply a `MessageChangeBatch` v1
(`contracts/message-change-batch.v1.schema.json`, stored as a blob, the job
holding only the inbox and the blob reference) in order, idempotently per
`(inbox, provider_message_id)`: an unknown message is inserted and threaded,
a known one has its read, starred, mailbox state and labels updated, and a
removed one is deleted with its blobs detached. Bodies, attachments and the
message's participants document SHALL be attached to the message in the
transaction that writes it, and the events SHALL be published in that same
transaction. A batch for an inbox that does not exist yet SHALL be retried; a
batch of another version or naming another inbox SHALL fail for good.

#### Scenario: Applying the same batch twice
- **WHEN** the same batch is applied again
- **THEN** no message, conversation, decision or event is added

#### Scenario: A later batch changes flags and removes a message
- **WHEN** a batch marks a message read and starred, takes another out of the inbox and removes a third
- **THEN** each affected conversation is recounted, one with no messages left is deleted, and `conversations.conversation_changed` is published with origin `provider`

#### Scenario: The inbox is not there yet
- **WHEN** a batch arrives for an inbox this module has not created
- **THEN** the job is retried rather than failed, and the batch blob stays held

#### Scenario: A malformed batch
- **WHEN** a batch does not parse
- **THEN** the job fails for good and its stored error names only the kind and position of the error, never content from the batch

### Requirement: Threading
A new message SHALL join, in order: the conversation with the provider's
thread key in its inbox; else the conversation of an ancestor named in its
`In-Reply-To` or `References`; else the conversation of a reply that arrived
before it and names it, merging any other such conversations into that one.
Otherwise it SHALL start a conversation. Conversations SHALL never span
inboxes, and subjects SHALL never be used to thread.

#### Scenario: Replies stored before their parent
- **WHEN** two replies to a message arrive before it, then the message, then a reply to one of the replies
- **THEN** all four are in one conversation, and the merged-away conversation is announced as removed

### Requirement: Events without personal data
`conversations.message_received` (inbound) and `conversations.message_sent`
(outbound) SHALL be published the first time a message is stored, carrying
ids, the channel, the stream, the sent time, the attachment count and blob
references only. Addresses, names and the subject SHALL be in the message's
participants document (`ParticipantsDocument` v1), a blob the events point at.

#### Scenario: A message with an address and a subject
- **WHEN** a message from `maya@northwind-studio.example` about "Q4 plan" is stored
- **THEN** no published event contains an address or the subject, and the participants document it points at contains both

### Requirement: Streams from accepted answers
Every conversation SHALL be in exactly one stream: `primary`, `updates`,
`receipts` or `newsletters`. A new conversation's stream SHALL be set from
its first inbound message, only from an answer to the `stream` question that
the policy accepts; otherwise it SHALL be `primary`. The shipped policy
(`Policy::baseline`, from the decide eval baseline) SHALL let no stream answer
act, since no strategy holds the stream target yet. Later messages SHALL
never re-sort a conversation, and a member's move SHALL stick.

#### Scenario: No rules strategy
- **WHEN** the module runs with the rules strategy that abstains
- **THEN** every conversation is in `primary` with stream source `default`

#### Scenario: The shipped policy
- **WHEN** the rules answer `newsletters` for a message with mailing-list headers under `Policy::baseline`
- **THEN** the conversation stays in `primary` and the answer is stored as a suggestion (`acted = false`)

#### Scenario: A policy that lets rules place the stream
- **WHEN** the policy accepts rules stream answers from 0.8 and the rules answer `newsletters` at 0.95
- **THEN** the conversation is in `newsletters` with stream source `rules`, and the answer is stored as acted on with its reasons

### Requirement: The decide pass records suggestions
Every answer of every strategy SHALL be stored in `decisions` with its
question, source, value, options, probabilities, confidence, model, latency,
reasons, whether it acted, and the envelope version. The rules SHALL be the
shared email signals and the questions the shared triage questions of
`campbooks_ai::decide`; this module SHALL hold no rules of its own. The model pass
(`conversations.decide_message`, role `decide`) SHALL ask the six questions
(`stream`, `needs_reply`, `asks_action`, `has_deadline`, `urgency`,
`suspicious`) in one call per inbound message, only when the `decide` role is
configured, only for messages sent within
`CAMPBOOKS_CONVERSATIONS__DECIDE_BACKFILL_DAYS` (default 14), and once per
message. Answers the policy accepts SHALL be marked `acted`; no model answer
SHALL be accepted by the shipped policy. Provider
failures SHALL be retried, recording only the kind of failure.

#### Scenario: A confident but wrong model answer
- **WHEN** the model answers `primary` at 0.97 for a newsletter the rules placed in `newsletters`
- **THEN** the answer is stored with `acted = false` and the conversation stays in `newsletters`

#### Scenario: Old mail
- **WHEN** a batch brings mail sent before the backfill window
- **THEN** the model is not called

#### Scenario: A replayed delivery
- **WHEN** `message_received` is delivered again to `conversations.decide_message`
- **THEN** the model is not asked again

### Requirement: Reading conversations
`GET /api/v1/w/{workspace_id}/inboxes` (`conversations.list_inboxes`) SHALL
list the caller's inboxes with their role and unread count.
`GET …/conversations` (`conversations.list`) SHALL list the caller's
conversations newest first, by stream and inbox, in the inbox or archived,
with an opaque cursor and a limit of 1 to 100 (else 400).
`GET …/conversations/{conversation_id}` (`conversations.get`) SHALL return the
conversation, up to its 100 newest messages with their plain-text bodies, and
the decide pass's answers about its newest inbound message.
`GET …/messages/{message_id}/body` (`conversations.get_message_body`) SHALL
return the text and the sanitised HTML bodies as JSON; HTML SHALL never be
served as `text/html`.

#### Scenario: Paging
- **WHEN** a member lists three conversations two at a time
- **THEN** the first page carries a `next_cursor` and the second page holds the oldest conversation and none

#### Scenario: A malformed cursor
- **WHEN** the cursor is not one the API gave
- **THEN** the response is 400 with code `invalid_cursor`

### Requirement: Tidying conversations
`conversations.mark_read` / `mark_unread`, `star` / `unstar`,
`archive` / `unarchive` and `add_label` / `remove_label` SHALL be undo pairs,
and `move_to_stream` SHALL be its own undo. Each SHALL change only what is not
already so, and when it changes something SHALL publish
`conversations.conversation_changed` with origin `member` and
`conversations.changes_requested` (`contracts/conversations.changes_requested.v1.schema.json`)
listing each affected message's provider id and the change kind. Changes that
came from the provider SHALL never produce `changes_requested`.

#### Scenario: Marking read twice
- **WHEN** a member marks a conversation read, then read again
- **THEN** one `changes_requested` with a `mark_read` item per previously unread message is published, and none the second time

#### Scenario: Removing a provider label
- **WHEN** a member takes off a label the provider knows
- **THEN** the `remove_label` item carries the label's provider key

### Requirement: Corrections
Moving a conversation to another stream SHALL record a correction: the
question, the stream it was in and the one chosen, the stream source it
overrode, the decisions on record for that question, the latest inbound
message and the member.

#### Scenario: Moving a newsletter to Primary
- **WHEN** a member moves a rules-placed newsletter to `primary`
- **THEN** a correction from `newsletters` to `primary` overriding `rules` is stored with the stream decision on record, and later batches leave it in `primary`

### Requirement: Inboxes from connections
When `connectors.capability_granted` is published for the `messages`
capability, the module SHALL create the inbox it names, with the address and
display name of the connection-account document it points at, and make the
granting member its `manager`. Other capabilities SHALL create nothing, and a
repeated event SHALL change nothing. When `connectors.messages_changed` is
published, the module SHALL enqueue the application of its batch, ordered
per inbox.

#### Scenario: A Gmail connection
- **WHEN** a `messages` grant names an inbox, then a batch is announced for it
- **THEN** the member sees the inbox as its manager, and the batch's conversations in it

#### Scenario: A calendar grant
- **WHEN** a `calendar` grant is published
- **THEN** no inbox is created

### Requirement: Change feed within inboxes
Conversation and message changes SHALL be declared to the change feed within
their inbox, and only members of that inbox (or a client token bound to the
workspace) SHALL see them.

#### Scenario: Archiving a conversation
- **WHEN** a member archives a conversation
- **THEN** the feed records an upsert of that conversation within its inbox

#### Scenario: Someone outside the inbox
- **WHEN** a member who is not in an inbox follows the feed
- **THEN** changes within that inbox are not shown to them

### Requirement: Workspace deletion
When `workspaces.workspace_deleting` is published, the module SHALL delete the
workspace's rows when its grace period ends.

#### Scenario: Purge
- **WHEN** a workspace is deleted with its grace period over
- **THEN** its inboxes, conversations and messages are gone and other workspaces' are kept

### Requirement: Sanitised email HTML
`conversations.get_message_body` SHALL sanitise the HTML body before
returning it: only an allow-list of tags and attributes survives; `script`
and `style` elements are removed with their content, as are forms, frames,
objects, `base` and `meta` elements, event-handler attributes and comments;
links keep only `http`, `https`, `mailto` and `tel` URLs, relative URLs are
dropped, and every link opens in a new tab with `rel="noopener noreferrer"`;
`style` attributes keep only declarations that load nothing (no `url(`,
`image-set(`, `@import`, `expression`, `behavior`, `-moz-binding` or CSS
escapes). Images of at most 2×2 pixels, or hidden by their style, SHALL be
removed. Other remote images SHALL lose their source unless the request asks
`show_images=true`, and the response SHALL say how many remote images the
message has (`remote_images`) and whether they were kept (`images_shown`).

#### Scenario: A newsletter with a script, a style sheet and a tracking pixel
- **WHEN** a member reads the HTML body of a message that has a `<script>`, a `<style>` with a remote `url()`, an `onclick` handler, a 1×1 image and a remote logo
- **THEN** the HTML has no script, style sheet, handler or tracking pixel, the logo has no source, and `remote_images` is 1

#### Scenario: Show images
- **WHEN** the same body is read with `show_images=true`
- **THEN** the logo keeps its `https` source, the tracking pixel is still removed, and `images_shown` is true

### Requirement: Stream counts
`GET /api/v1/w/{workspace_id}/streams` (`conversations.list_streams`,
`conversations:read`) SHALL list the four built-in streams in display order,
each with the number of the caller's conversations in it that are still in
the inbox and how many of those have an unread message, optionally for one
inbox (`inbox`, 404 when the caller cannot see it). Streams with nothing in
them SHALL be listed with zero.

#### Scenario: Counting
- **WHEN** a member with three conversations, one unread in Primary, lists the streams
- **THEN** the response lists primary, updates, receipts and newsletters in that order, their conversations add up to three, and Primary has one unread

#### Scenario: Someone else's inbox
- **WHEN** a member who is not in an inbox lists the streams
- **THEN** its conversations are not counted

### Requirement: Suggestions with their reasons
A conversation summary SHALL carry `suggestion` when the newest stream answer
on record for it (the model's before the rules' for the same message) did
not act, names another stream than the conversation's, and no member placed
the conversation; the suggestion SHALL carry the stream, its source, its
confidence (0 to 100, when known) and its stored reasons. A conversation's
detail SHALL carry `decisions`: for each question, the answer about its newest
inbound message, with its source, confidence, reasons and whether it acted.
Neither SHALL change where a conversation is.

#### Scenario: The shipped policy
- **WHEN** the rules answer `newsletters` for a message under `Policy::baseline`
- **THEN** the conversation stays in `primary`, its summary suggests `newsletters` with the rules' reasons, and its detail lists the stream answer with `acted = false`

#### Scenario: A member moved it
- **WHEN** a member moves that conversation to `newsletters`
- **THEN** the summary the move returns has no suggestion

### Requirement: Sending scope and roles
The `conversations` module SHALL declare the scope `conversations:send`, and
every draft operation SHALL require it. A user SHALL also need the `sender`
or `manager` role on the draft's inbox: a `reader` SHALL get 403
`inbox_read_only`, and a user who is not a member of the inbox SHALL get 404.

#### Scenario: A token without the scope
- **WHEN** a client with only `conversations:read conversations:write` creates a draft
- **THEN** the response is 403 as `application/problem+json`

#### Scenario: The composer
- **WHEN** a member presses Reply, Reply all or Forward in the reader, or `c` for a new message
- **THEN** a composer opens From the inbox, with To, Cc and Bcc as chips, the quoted history folded under the text, forwarded attachments listed, ⌘↵ to send, ⌘B and ⌘I to format and Esc to close, and Undo in a toast after sending

#### Scenario: A reader replies
- **WHEN** a member with the `reader` role on an inbox creates a reply draft to one of its messages
- **THEN** the response is 403 with code `inbox_read_only` and no draft exists

### Requirement: Drafts
A draft SHALL belong to one inbox and to its author, and SHALL be visible
only to its author (a client token: only to client tokens); anyone else
SHALL get 404. Creating a reply, reply-all or forward draft of a message
SHALL fill on the server: the recipients (reply: the message's `Reply-To`,
else its sender, or for a message the inbox sent its `To`; reply all: also
its `To` and `Cc`, never the inbox's own address, no duplicates; forward:
none), the subject with `Re: ` or `Fwd: ` unless already there, the
`In-Reply-To`, the `References` (the message's plus its `Message-ID`, at most
20), the provider thread key, and for a forward every non-inline attachment
of the message, whose blobs the draft holds. A draft SHALL be saved by
partial updates that name the `version` they change; a stale version SHALL
get 409 `draft_changed`.

#### Scenario: Reply all leaves the inbox out
- **WHEN** Sam replies all to a message from Maya to Sam and Leo, copying Ana
- **THEN** the draft is to Maya and copies Leo and Ana, and Sam's own address is in neither list

#### Scenario: Forward keeps the attachments
- **WHEN** Sam forwards a message with a PDF attachment
- **THEN** the draft has no recipients, its subject starts with `Fwd: `, and it lists the PDF, which stays stored even if the original message is removed

#### Scenario: Two saves of the same version
- **WHEN** two updates name the same version
- **THEN** the first is saved and the second answers 409 `draft_changed`

#### Scenario: Another member's draft
- **WHEN** a second sender of the inbox reads Sam's draft
- **THEN** the response is 404

### Requirement: Sending with undo
`conversations.send_draft` SHALL name the draft version it sends and SHALL
answer 409 `draft_changed` when that is not the saved one, so what is sent
is what the member last saw saved; the client SHALL send only once its last
save of what is on screen succeeded. It SHALL refuse a draft with no recipient (400
`no_recipients`), a malformed address (400 `invalid_address`) or more than
25 MiB of attachments (400 `attachments_too_large`); otherwise it SHALL schedule the
draft and answer 202 with the time it will be released, after
`CAMPBOOKS_CONVERSATIONS__UNDO_SEND_SECONDS` (default 10). Until then
`conversations.cancel_send` SHALL put it back to an editable draft; after
release it SHALL answer 409 `too_late`. A scheduled or sending draft SHALL
not be edited, deleted or sent again (409 `draft_not_editable`).

#### Scenario: Undo within the window
- **WHEN** Sam sends a reply and cancels it before the window ends
- **THEN** the draft is editable again and nothing is ever published for that send

#### Scenario: Sending again after an undo
- **WHEN** Sam cancels a send, edits the draft and sends it again
- **THEN** only the second send is released, once

### Requirement: The outbound intent
When the window ends, the job `conversations.release_send` SHALL write an
outgoing message document (`contracts/outgoing-message.v1.schema.json`, a
blob holding the addresses, subject, plain text, the HTML rendered from the
text's basic formatting with everything else escaped, and the forwarded
attachments' blob references), mint a send id and a `Message-ID`, and
publish `conversations.send_requested` v1
(`contracts/conversations.send_requested.v1.schema.json`, ids and the blob
only) in the transaction that marks the draft as sending.

#### Scenario: Formatting cannot inject HTML
- **WHEN** the body contains `<script>` and `**done**`
- **THEN** the HTML part shows `&lt;script&gt;` as text and `done` in bold

### Requirement: The send outcome
On `connectors.send_completed` v1
(`contracts/connectors.send_completed.v1.schema.json`), the consumer
`conversations.record_send` SHALL delete a sent draft, releasing its blobs,
and publish `conversations.draft_sent`; or mark a failed draft `failed` with
the reason and publish `conversations.send_failed`. A failed draft SHALL be
editable and may be sent again. The sent message SHALL reach its
conversation through the connector's change batch, like any message. A
released send that hears nothing back for an hour SHALL fail with
`unknown_outcome`, and a late outcome for it SHALL still be recorded.

#### Scenario: The reply appears in its conversation
- **WHEN** a reply is sent
- **THEN** the conversation gains one outbound message and the draft is gone

#### Scenario: Gmail refuses the message
- **WHEN** the connector reports `rejected`
- **THEN** the draft is `failed` with reason `rejected` and can be edited and sent again

### Requirement: Conversations as a search source
The module SHALL implement the kernel's `SearchSource` port for the group
type `conversations.conversation`: a conversation's container (its inbox),
time (its last message), subject, label names, facets (stream, unread,
starred, archived, label ids) and, per message, its people (from, to, cc
with names and addresses), plain text (the text part, or the HTML part
reduced to text when there is none, quoted history removed, at most
32 KiB), attachment names, mailbox (`inbox`, `archive`, `spam`, `trash`),
direction and a content stamp made of the hashes of the blobs it is read
from. A conversation that no longer exists SHALL be reported as gone.
Drafts SHALL NOT be returned.

#### Scenario: HTML-only message
- **WHEN** search asks for a message that has only an HTML part
- **THEN** it gets the message's text without tags, scripts or styles

#### Scenario: Quoted history
- **WHEN** a reply quotes the previous message under "On Tue, Ana wrote:"
- **THEN** the text handed to search ends before the quote

#### Scenario: A flag change
- **WHEN** a conversation is marked read and search refreshes it
- **THEN** the content stamps of its messages are unchanged, so no body is read again

### Requirement: Visible inboxes as a list
The module's container-visibility port SHALL also list the inboxes a caller
may see (a user: the inboxes they are a member of; a client token: every
inbox of the workspace), as `conversations.inbox` record references, and
SHALL list nothing for other container types.

#### Scenario: A member's inboxes
- **WHEN** the port lists `conversations.inbox` containers for a member of two of a workspace's three inboxes
- **THEN** it returns those two
