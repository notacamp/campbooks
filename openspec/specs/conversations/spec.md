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
the policy accepts; otherwise it SHALL be `primary`. The facts never answer
`stream`, so a conversation is stored in `primary` and the model pass places
it: when the policy accepts the model's `stream` answer for the
conversation's first inbound message and the conversation's stream source is
still `default`, or `rules` (a placement by the content rules retired in
ADR 0018), the conversation SHALL move to that stream with stream source
`model`. Later messages SHALL never re-sort a conversation, and a member's
move SHALL stick.

#### Scenario: No rules strategy
- **WHEN** the module runs with no `decide` role
- **THEN** every conversation is in `primary` with stream source `default`

#### Scenario: The shipped policy
- **WHEN** the facts strategy runs on a message with mailing-list headers
- **THEN** it gives no `stream` answer and the conversation is stored in `primary`

#### Scenario: A policy that lets rules place the stream
- **WHEN** a test policy accepts rules stream answers from 0.8 and a test rules strategy answers `newsletters` at 0.95
- **THEN** the conversation is in `newsletters` with stream source `rules`, and the answer is stored as acted on with its reasons

#### Scenario: The model sorts a newsletter
- **WHEN** the policy accepts model stream answers from 0.9 and the model answers `newsletters` at 0.98 for a conversation's first inbound message
- **THEN** the conversation is in `newsletters` with stream source `model`, and the answer is stored as acted

#### Scenario: A placement by the retired rules gives way
- **WHEN** the content rules placed a conversation in `receipts` before the upgrade, and the model pass answers `newsletters` for its first inbound message with an accepted confidence
- **THEN** the conversation is in `newsletters` with stream source `model`

#### Scenario: A member's move sticks
- **WHEN** a member moved a conversation to `primary` before the model pass answers `newsletters` with an accepted confidence
- **THEN** the conversation stays in `primary` and the answer is stored with `acted = false`

### Requirement: The decide pass records suggestions
Every answer of every strategy SHALL be stored in `decisions` with its
question, source, value, options, probabilities, confidence, model, model
version, latency, reasons, whether it acted, and the envelope version
(`1 + CONTEXT_VERSION` of the triage context). The rules SHALL be the
shared facts strategy and the questions the shared triage questions of
`campbooks_ai::decide`; this module SHALL hold no rules of its own. The model pass
(`conversations.decide_message`, role `decide`) SHALL ask the six questions
(`stream`, `needs_reply`, `asks_action`, `has_deadline`, `urgency`,
`suspicious`) in one call per inbound message, telling the model whose inbox
the message is in and the facts as hints, only when the `decide` role is
configured and has passed its answers check, only for messages sent within
`CAMPBOOKS_CONVERSATIONS__DECIDE_BACKFILL_DAYS` (default 14), and once per
message. When the role has not passed its answers check, the pass SHALL do
exactly what it does with no `decide` role: store nothing, fail no job.
Answers the policy accepts SHALL be marked `acted`; the shipped policy
(`Policy::baseline`) SHALL let a model answer act from the operating point
where the committed eval report shows the model run holding its question's
target. Provider failures SHALL be retried, recording only the kind of
failure.

#### Scenario: A confident but wrong model answer
- **WHEN** the policy accepts model `needs_reply` answers from 0.9 and the model answers true at 0.7
- **THEN** the answer is stored with `acted = false`

#### Scenario: Old mail
- **WHEN** a batch brings mail sent before the backfill window
- **THEN** the model is not called

#### Scenario: A replayed delivery
- **WHEN** `message_received` is delivered again to `conversations.decide_message`
- **THEN** the model is not asked again

#### Scenario: A model that failed its answers check
- **WHEN** the decide role's last answers check failed and a batch of mail arrives
- **THEN** the model is not called, and the decisions, streams and needs are the same as with no `decide` role

#### Scenario: The recipient
- **WHEN** a message arrives in Sam's inbox, which only Sam reads
- **THEN** the model reads "The recipient is sam@example.com, who reads this personal inbox."

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
removed. Other remote images SHALL have their source replaced by the
server's placeholder image (a fixed `data:` URL; their size and alt text
kept) unless the request asks `show_images=true`, and the response SHALL say
how many remote images the message has (`remote_images`) and whether they
were kept (`images_shown`). An image whose source is `cid:<id>` SHALL be
shown, whether or not images are shown, as a `data:` URL of the message's
inline part with that `Content-ID` (at most 255 characters; a longer one
is not stored) when that part has a raster image type its bytes confirm
and fits the size limits, and as the placeholder otherwise. No other `data:`
source survives.

#### Scenario: A newsletter with a script, a style sheet and a tracking pixel
- **WHEN** a member reads the HTML body of a message that has a `<script>`, a `<style>` with a remote `url()`, an `onclick` handler, a 1×1 image and a remote logo
- **THEN** the HTML has no script, style sheet, handler or tracking pixel, the logo's source is the placeholder, and `remote_images` is 1

#### Scenario: Show images
- **WHEN** the same body is read with `show_images=true`
- **THEN** the logo keeps its `https` source, the tracking pixel is still removed, and `images_shown` is true

#### Scenario: An inline logo
- **WHEN** a member reads a message whose HTML shows `cid:logo@example.com` and which has an inline PNG part with that content id
- **THEN** the image's source is a `data:image/png;base64,…` URL of that part, without asking to show images, and it is not counted in `remote_images`

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
on record for it (the model's before the rules' for the same message; a
model's answer only when its `model_version` starts with `checked:`) did
not act, names another stream than the conversation's, and no member placed
the conversation; the suggestion SHALL carry the stream, its source, its
confidence (0 to 100, when known) and its stored reasons. A conversation's
detail SHALL carry `decisions`: for each question, the answer about its newest
inbound message, with its source, confidence, reasons and whether it acted,
read the same way. Model answers without that stamp SHALL be kept and never
read. Neither SHALL change where a conversation is.

#### Scenario: The shipped policy
- **WHEN** the rules answer `newsletters` for a message under `Policy::baseline`
- **THEN** the conversation stays in `primary`, its summary suggests `newsletters` with the rules' reasons, and its detail lists the stream answer with `acted = false`

#### Scenario: A member moved it
- **WHEN** a member moves that conversation to `newsletters`
- **THEN** the summary the move returns has no suggestion

#### Scenario: Model answers from before the answers check
- **WHEN** a message's model answers carry `model_version` `laya-0.3.21-unchecked`
- **THEN** its detail lists the rules' answers, and the model's answers stay stored

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

### Requirement: Attachment downloads
`conversations.get_attachment` (`GET
/api/v1/w/{workspace_id}/conversations/{conversation_id}/messages/{message_id}/attachments/{attachment_id}`,
scope `conversations:read`) SHALL stream an attachment's stored bytes to a
caller who can see the message's inbox, and SHALL answer 404 when the
conversation, message and attachment ids do not belong together, when the
caller cannot see the inbox, or when the bytes are gone. The response SHALL
carry `Content-Disposition: attachment` with a sanitised file name (no path
separators, control or bidirectional-override characters, no leading dots,
at most 150 characters, `attachment` when nothing is left) as both an ASCII
`filename` and a UTF-8 `filename*`; the stored content type only when it is
on the safe list, else `application/octet-stream`; `X-Content-Type-Options:
nosniff`; `Content-Security-Policy` starting with `sandbox`;
`Cross-Origin-Resource-Policy: same-origin`; `Cache-Control: private,
no-store`; and `Accept-Ranges: none`. A raster image or PDF SHALL be served as the type its first bytes show,
and as `application/octet-stream` when they show neither. With
`inline=true` the disposition SHALL be `inline` only for raster images and
PDFs; every other type stays an attachment.

#### Scenario: A member downloads a PDF
- **WHEN** a member of the inbox requests a message's PDF attachment
- **THEN** the answer is 200 with the stored bytes, `Content-Type: application/pdf`, `Content-Disposition: attachment; filename="…"; filename*=UTF-8''…`, `nosniff` and a `sandbox` policy

#### Scenario: Another inbox
- **WHEN** a member of the workspace who is not in the message's inbox requests the attachment
- **THEN** the answer is 404, the same as for an id that does not exist

#### Scenario: An HTML attachment
- **WHEN** a member requests an attachment stored as `text/html`, with `inline=true`
- **THEN** it is served as `application/octet-stream` with `Content-Disposition: attachment`

#### Scenario: A malicious file name
- **WHEN** an attachment is named `../../etc/pass\r\nwd‮gpj.exe`
- **THEN** the header names it `etcpasswdgpj.exe`, with no separators, line breaks or bidirectional controls, as one valid header

### Requirement: The need of a conversation
The module SHALL keep at most one open need per conversation, derived from the
newest inbound message sent within
`CAMPBOOKS_CONVERSATIONS__NEEDS_WINDOW_DAYS` (default 14), its decisions, the
policy and the conversation's state. A `sure` need SHALL come only from
answers the policy accepts: `pay` when `asks_action` is accepted and true and
the message's accepted `stream` answer is `receipts`, else `reply` when
`needs_reply` is accepted and true, else `do` when `asks_action` is accepted
and true. A `reply` or `do` need, `sure` or `check`, SHALL be raised only
when the message's accepted `stream` answer is `primary`, or, when it is
`updates` or no `stream` answer is accepted, when an accepted `has_deadline`
answer is true; a message sorted to `newsletters` SHALL raise neither. When
answers of more than one source to the same question were
accepted, the facts' answer SHALL be the one used. A `suspicious` answer of
true that the policy accepts SHALL raise a `review` need instead of any
other. When the `decide` role is configured, a model answer of true for
`needs_reply` or `asks_action` that the policy does not accept, with a
confidence of at least 0.8, where no `sure` need exists and no accepted
answer to the same question is false, SHALL raise a `review` need with trust
`check`. Only model answers whose `model_version` starts with `checked:`
SHALL count. A need SHALL carry no due date, and `urgent` when an accepted
`urgency` answer is "today". Messages from the conversation's own inbox SHALL
never raise one.

#### Scenario: A question from a person
- **WHEN** Marta writes "Could you confirm Thursday's walkthrough?" and the policy accepts the model's `needs_reply` true
- **THEN** the conversation has a `sure` `reply` need with no due date

#### Scenario: A bill
- **WHEN** the policy accepts the model's `stream` answer `receipts` and its `asks_action` true for an invoice from a service
- **THEN** the need is `pay`

#### Scenario: An invoice sent by hand
- **WHEN** the policy accepts the model's `stream` answer `primary` and its `asks_action` true for an invoice a supplier attached to their own email
- **THEN** the need is `do`

#### Scenario: A notification with a question
- **WHEN** a shipping notice with a `List-Unsubscribe` header ends "Questions? Contact our support team." and the model answers `needs_reply` true
- **THEN** the facts' accepted "no" wins and no `reply` need is raised

#### Scenario: A notification that reads like an ask
- **WHEN** the policy accepts the model's `stream` answer `updates` and its `asks_action` true for a calendar invitation, and its `has_deadline` answer is false
- **THEN** no need is raised

#### Scenario: A notification asking by a date
- **WHEN** the policy accepts the model's `stream` answer `updates`, its `asks_action` true and its `has_deadline` true for a delivery to reschedule
- **THEN** the need is `do`

#### Scenario: A newsletter
- **WHEN** the policy accepts the model's `stream` answer `newsletters` and its `asks_action` true
- **THEN** no need is raised

#### Scenario: Phishing asks for an action
- **WHEN** the policy accepts the model's `suspicious` true and `asks_action` true for a message
- **THEN** the need is `review` and no `do` need exists

#### Scenario: Old mail on the first sync
- **WHEN** a first sync brings a question sent 40 days ago
- **THEN** no need is raised

#### Scenario: Rules only
- **WHEN** no `decide` role is configured
- **THEN** no need has trust `check` and no `sure` need is raised

#### Scenario: The rules already settled it
- **WHEN** the facts accept `needs_reply` false for a code host's list notification, and the model answers `needs_reply` true with confidence 0.95
- **THEN** no `reply` need is raised

#### Scenario: Only the model says yes
- **WHEN** no answer to `asks_action` is accepted and the model answers it true with confidence 0.875, below the policy's operating point
- **THEN** the need is `review` with trust `check`

#### Scenario: Unchecked model answers
- **WHEN** a message's only `needs_reply` true is a model answer marked `laya-0.3.21-unchecked`
- **THEN** reading it raises no `check` need, and an open `check` need it raised before is withdrawn as `superseded` on the next refresh

### Requirement: Clearing a need
A need SHALL be cleared, with its cause, when: a message is sent from the
inbox in the conversation after the message that raised it (`replied`; clears
`reply` and `do`), the conversation is archived (`archived`; clears `reply` and
`review`), every message is in the trash or spam or removed (`trashed`; clears
any), or a member resolves it. A new inbound message SHALL derive the need
again from that message (`superseded` for the old one). When the open need's
own message, read again, raises no need and no member confirmed it, the need
SHALL be cleared as `superseded`. Changes SHALL come from the provider or
from members alike.

#### Scenario: Replied from the phone
- **WHEN** Ana replies to Marta from Gmail on her phone and the sent message reaches the conversation
- **THEN** the `reply` need is cleared with cause `replied`

#### Scenario: Archiving a bill
- **WHEN** a member archives a conversation with a `pay` need
- **THEN** the need stays open until a member chooses Done or Not needed

#### Scenario: Done in a shared inbox
- **WHEN** Ana chooses Done on a need in an inbox she shares with Leo
- **THEN** the need is cleared for both of them

#### Scenario: A question raised against a settled answer
- **WHEN** a conversation has an open `check` need whose message's accepted answers say it asks nothing, and the conversation's need is refreshed
- **THEN** the need is cleared with cause `superseded`, announced once, and not shown under Done today

### Requirement: Need changes are published
Every change to a need SHALL publish `conversations.need_changed` v1
(`contracts/conversations.need_changed.v1.schema.json`) in its transaction,
carrying the workspace, inbox, conversation and message ids, a revision that
grows with every change of that conversation's need, the message's sent time,
the need (reason, trust, due date, urgent) or null, the cause and the
member when cleared, and the conversation's newest inbound message. A newer
inbound message SHALL be announced even when the need does not change. It SHALL carry no names, addresses, subjects or text, and
SHALL declare an upsert of the conversation within its inbox.

#### Scenario: Cleared by a member
- **WHEN** Ana chooses Done on a need
- **THEN** `need_changed` carries a null need, cause `done`, Ana's user id and a revision one higher than before

### Requirement: Resolving a need
`POST …/conversations/{conversation_id}/need/resolve`
(`conversations.resolve_need`, `conversations:write`) SHALL clear the open
need with outcome `done` or `not_needed`, naming the message the need the
member saw comes from (`message_id`): when the open need comes from another
message, nothing SHALL be recorded and the answer SHALL be 409
`need_changed` (the same for `confirm_need`); `not_needed` SHALL also store a
correction of the question that raised it (true to false). `…/need/confirm`
(`conversations.confirm_need`) SHALL turn a `check` need into a `sure` one
and store a correction confirming the answer. `…/need/reopen`
(`conversations.reopen_need`) SHALL undo either, restoring the need as it was
and marking the correction withdrawn. Each SHALL take the `sender` role or
above (a `reader` gets 403 `inbox_read_only`), and SHALL answer 409
`no_open_need` when there is nothing to resolve or confirm, and
`nothing_to_reopen` when there is nothing to reopen.

#### Scenario: Not needed teaches
- **WHEN** a member marks a rules `reply` need Not needed
- **THEN** the need is cleared with cause `not_needed` and a correction of `needs_reply` from true to false overriding `rules` is stored

#### Scenario: Undo
- **WHEN** the member then reopens it
- **THEN** the same need is open again and the correction is marked withdrawn

#### Scenario: The need changed meanwhile
- **WHEN** a member resolves a need naming a message other than the one the open need comes from
- **THEN** the response is 409 `need_changed` and no correction is stored

#### Scenario: It's safe
- **WHEN** a member marks a `review` need for a suspicious message Not needed
- **THEN** a correction of `suspicious` from true to false is stored and no other need is raised for that message

### Requirement: The sender's sentence
The email signals SHALL keep, with a reason whose evidence comes from the
message's text, the sentence of the new part of the message (quoted history
excluded) that holds the evidence, with whitespace collapsed, without
bidirectional controls, zero-width or control characters, and at most 240
characters, cut at a word. It SHALL be stored with the decision's reasons and
SHALL NOT change any answer.

#### Scenario: A reversed IBAN
- **WHEN** a payment request hides a right-to-left override and a zero-width space inside an IBAN
- **THEN** the quoted sentence holds neither, and the IBAN reads in order

#### Scenario: A request inside a long paragraph
- **WHEN** a message says "Hope the move went well. Could you confirm Thursday's walkthrough? Thanks!"
- **THEN** the `reply_asked` reason's sentence is "Could you confirm Thursday's walkthrough?"

### Requirement: Due dates from words
The email signals SHALL resolve a deadline's words to a date relative to the
message's sent date (in UTC until accounts carry a time zone): a weekday is
the next such day after the sent date, "today", "tomorrow" and "end of the
week/month" their dates, and a numeric date day-first. Words that do not
resolve SHALL leave the need with no due date and keep the words in the
reason.

#### Scenario: Thursday
- **WHEN** a message sent on Monday 12 October says "by Thursday"
- **THEN** the due date is Thursday 15 October

#### Scenario: Unclear words
- **WHEN** a message says "before the holidays"
- **THEN** the need has no due date and its reason's evidence is "before the holidays"

### Requirement: Needs in summaries
A conversation summary SHALL carry `need` while its need is open: the reason,
trust, due date, whether it is overdue, whether it is urgent, the rule of the
reason that decided it, and the sender's sentence with the sender's name and
the message's sent time. `conversations.list` SHALL accept `ids` (1 to 100
conversation ids, else 400) and return those the caller can see, leaving the
others out.

#### Scenario: Home asks for its rows
- **WHEN** the client lists three conversation ids, one of them in an inbox the caller cannot see
- **THEN** two summaries are returned, each with its `need`

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

### Requirement: Taught rules
The module SHALL store taught rules: deterministic condition → action pairs
for one inbox, with one to four conditions that must all hold and one to four
actions, at most one per slot (`stream`, `archive`, `read`, `needs_reply`,
`asks_action`, `surface`; `add_label` is additive). Conditions SHALL be
`sender_address is`, `sender_domain is` (never a free-mail domain on its
own), `list_id is`, `subject_template is`, `subject_contains` (whole words,
case- and accent-insensitive), `header is|present` (a stored header),
`label has` and `stream is`. Actions SHALL be `move_to_stream`, `archive`,
`add_label`, `mark_read`, `needs_reply` true or false, `asks_action` true or
false and `surface` always or never. A rule SHALL record its origin
(`action`, `onboarding`, `instruction`, `correction`, `suggestion`), the
member's words for an instruction, its author, and its state (`proposed`,
`active`, `paused`, `removed`). Taught rules are member data: the built-in
email signals stay the module's only rules of its own.

#### Scenario: A rule from a move
- **WHEN** Ana creates a rule with `sender_address is notifications@github.com` and `move_to_stream updates`
- **THEN** it is stored active for her inbox with origin `action` and Ana as its author

#### Scenario: A free-mail domain alone
- **WHEN** a rule's only condition is `sender_domain is gmail.com`
- **THEN** the response is 400 with code `invalid_rule` and nothing is stored

#### Scenario: Two actions in one slot
- **WHEN** a rule asks to move to `updates` and to `receipts`
- **THEN** the response is 400 with code `invalid_rule`

### Requirement: Subject templates
Every message SHALL keep a subject template: the subject lower-cased,
without `Re:`/`Fwd:` prefixes in the four locales, with dates and digit runs
replaced by `#`, tokens of six or more characters mixing letters and digits
replaced by `*`, spaces collapsed and at most 120 characters, or none when
fewer than two literal words remain. Existing messages SHALL be backfilled.

#### Scenario: An order notice
- **WHEN** a message's subject is "Re: Your order #48213 has shipped"
- **THEN** its template is "your order # has shipped"

#### Scenario: Nothing to keep
- **WHEN** a subject is "Invoice 2026-10-03"
- **THEN** it has no template

### Requirement: Rules run first and always the same
For each new inbound message, the module SHALL evaluate the inbox's active
rules after the built-in signals and before placing the conversation. Per
slot, the matching rule with the highest specificity (template, List-Id or
header 3; subject words or sender address 2; domain, label or stream 1;
summed) SHALL win, then the latest confirmed, then the lowest id. A rule's
answers to `stream`, `needs_reply` and `asks_action` SHALL be stored in
`decisions` with source `taught`, the rule as the reason and `acted = true`,
and SHALL override the signals' answers to the same question; the model pass
SHALL never override them. Its `archive`, `mark_read` and `add_label`
actions SHALL apply to the new message. A stream placed by a rule SHALL have
stream source `taught`, which a member's move overrides and nothing else
does. The same message and the same rules SHALL always give the same outcome.

#### Scenario: A new notification
- **WHEN** a message from notifications@github.com arrives and an active rule moves that sender's mail to `updates`
- **THEN** the new conversation is in `updates` with stream source `taught`, and the stream decision has source `taught` and acted

#### Scenario: The more specific rule
- **WHEN** one rule moves mail from shop.example to `receipts` and another moves "your order # has shipped" from shop.example to `updates`
- **THEN** a shipping notice from shop.example goes to `updates` and an invoice from it goes to `receipts`

#### Scenario: A rule beats the signals
- **WHEN** the signals answer `needs_reply` true at 0.95 and an active rule answers it false for that sender
- **THEN** no reply need is raised

### Requirement: Suspicious mail is never hidden by a rule
When a message has a true `suspicious` answer from any source, or its
conversation has an open Review need of any kind (a warning or a question to
the member), rules SHALL NOT archive it, mark it
read, move it out of `primary`, answer a need question false, or apply
`surface: never`; the Review need SHALL stay. Such a conversation SHALL
never start a rule suggestion.

#### Scenario: A look-alike sender
- **WHEN** a phishing message claims to be from notifications@github.com and the rules answer `suspicious` true
- **THEN** the archive and move rules for that sender leave it in Primary with its Review need

### Requirement: Surfacing
A rule with `surface: never` SHALL keep a need from being raised for the
mail it matches, and SHALL clear an open one with cause `rule`. A rule with
`surface: always` SHALL keep any other rule from archiving, marking read or
moving the conversation out of `primary`; it raises no need of its own in
this version.

#### Scenario: Never needs me
- **WHEN** a bill from billing@hosting.example arrives and a rule says `surface: never` for that sender
- **THEN** no need is raised for it

#### Scenario: Always show me
- **WHEN** one rule archives mail from mfiscal.example and a more specific one says `surface: always` for ines@mfiscal.example
- **THEN** a message from ines@mfiscal.example stays in the inbox, in Primary

### Requirement: Rule operations
`POST …/inboxes/{inbox_id}/rules/preview` (`conversations.preview_rule`,
`conversations:read`, not mutating: a read served by POST so the rule's
addresses and words stay out of URLs and access logs) SHALL answer, for
conditions and actions, how many conversations in the inbox the rule
matches and would change, up to five of them (subject, sender and date,
newest first), how many it would skip as suspicious or placed by a member,
and the active rules it would take over from with how many conversations.
`POST …/inboxes/{inbox_id}/rules` (`conversations.create_rule`,
`conversations:write`, undo `conversations.remove_rule`) SHALL store a rule
active or proposed and, when `apply_to_existing` is set for an active rule,
start a run. `POST …/rules/{rule_id}/confirm` (`conversations.confirm_rule`)
SHALL make a proposed rule active (with `apply_to_existing`), else answer 409
`rule_not_proposed`. `PATCH …/rules/{rule_id}` (`conversations.update_rule`)
SHALL change its conditions or actions, pause it, or let a manager adopt it.
`DELETE …/rules/{rule_id}` (`conversations.remove_rule`) SHALL remove it, and
with `restore=true` put back what it changed. `GET …/rules`
(`conversations.list_rules`, proposed rules first, removed ones left out) and
`GET …/rules/{rule_id}` (`conversations.get_rule`, a removed one too, so the
run putting its mail back can be followed) SHALL answer the rules of the
inboxes the caller can see with their counts, latest run and recent
exceptions. Conditions SHALL be stored normalised (trimmed; addresses, domains, list ids
and templates lower-cased) and a value SHALL be at most 320 characters. A
rule with the same conditions and an action of the same kind as an active
one SHALL replace it, cancelling its runs and announcing it removed, only
when the caller may change that rule; otherwise the response SHALL be 403
`rule_not_yours` and nothing changes. A proposed rule SHALL not be paused or
resumed (409 `rule_not_active`): only `confirm_rule` makes it act.
`from_conversation_id` SHALL name a conversation of the rule's inbox.

#### Scenario: Someone else's rule
- **WHEN** Ines, a sender of info@, creates a rule with the same conditions as Leo's, written " Notifications@GitHub.com "
- **THEN** the response is 403 `rule_not_yours` and Leo's rule stays active; a manager doing the same replaces it

#### Scenario: Preview
- **WHEN** a member previews moving mail from notifications@github.com to `updates` in an inbox with 1,000 such conversations in Primary, 2 of them suspicious
- **THEN** the preview would change 998, lists five of them and says 2 are skipped as suspicious

### Requirement: Previews and suggestions only read
A preview and a suggestion SHALL run in a read-only transaction whose
statements give up after five seconds, and SHALL look at no more than 20,000
conversations, saying `complete: false` when they stopped before the end of
the inbox. Their samples SHALL never show a suspicious conversation (it is
still counted). Messages stored before subject templates existed SHALL match
template conditions all the same (their template is worked out as they are
read), and `conversations.fill_templates` SHALL store those templates in the
background, a thousand messages per job, enqueued by the inbox's next change
batch or a new rule.

#### Scenario: A huge inbox
- **WHEN** a member previews a rule in an inbox of 300,000 conversations
- **THEN** the answer comes from the first 20,000 with `complete: false`, and nothing is written

#### Scenario: A proposed rule does nothing
- **WHEN** a rule is created as `proposed`
- **THEN** new mail it matches is not changed until a member confirms it

#### Scenario: Replacing
- **WHEN** a member creates a rule moving notifications@github.com to `newsletters` while one moves it to `updates`
- **THEN** the old rule is removed without putting mail back and the new one is active

### Requirement: Who may teach
Rules SHALL be the access catalogue's `conversations.rule`, inside their
inbox. Previewing, creating and confirming a rule, and asking for or
dismissing a suggestion, SHALL take the catalogue's `write` on the inbox (a
`sender` or a `manager`; a `reader`, an owner without a membership, or anyone
in an inbox kept read-only, gets 403 `inbox_read_only`). A proposed rule does
not act yet: confirming it and turning it down (`POST …/rules/{rule_id}/decline`,
`conversations.decline_rule`) SHALL take the same `write` on the inbox, and
confirming SHALL keep its author (the confirmer becomes it only when it had
none). Updating, removing and restoring an active rule SHALL take its author
while a `sender`, or a `manager` of the inbox; other writers get 403
`rule_not_yours`. Reading rules SHALL take any role on the inbox; a
rule in an inbox the caller cannot see SHALL answer 404. A client token, even
one a member registered (which arrives as that member), SHALL only read and
preview rules: creating, confirming, changing, removing or restoring one
answers 403 `rule_needs_member`. A rule SHALL act for every member of its
inbox. When its author no longer may write in the inbox, or the inbox is kept
read-only, the rule SHALL be paused (and its running runs stop) until a
manager adopts or removes it. While its author's account is suspended, the
rule SHALL be paused the same way (a running apply run is cancelled at its
next batch, new mail is not changed by it, and previews and suggestions do
not count it), and lifting the suspension SHALL make it act again with
nothing to redo; a run it cancelled is not restarted.

#### Scenario: Leo's rule in a shared inbox
- **WHEN** Leo taught a rule in info@ and Ana, a sender there, tries to remove it
- **THEN** the response is 403 `rule_not_yours`, and Ana's manager colleague can remove it

#### Scenario: The author leaves
- **WHEN** Leo is removed from info@
- **THEN** his rules there are paused and stop acting on new mail

#### Scenario: The author is suspended, then the suspension is lifted
- **WHEN** an administrator suspends Leo, who taught a rule in info@, and later lifts the suspension
- **THEN** while suspended the rule reads as paused with `paused_reason` `author_suspended` and leaves new mail alone, and once lifted it is active and acts on new mail again

#### Scenario: A run going when the author is suspended
- **WHEN** Leo's rule is being applied to existing mail and Leo is suspended before its next batch
- **THEN** that batch cancels the run and changes nothing

#### Scenario: An app
- **WHEN** a client token with `conversations:write`, registered by Sam, creates a rule
- **THEN** the response is 403 with code `rule_needs_member`

#### Scenario: Kept read-only
- **WHEN** the connection feeding an inbox is removed with its mail kept while a rule's run is going
- **THEN** the run is cancelled, the rule reads as paused, and creating a rule there answers 403 `inbox_read_only`

### Requirement: Rules from evidence ask first
A rule created with origin `correction` or `suggestion` SHALL be active only
when the kernel's `MemberPreferences` port says the creating member lets
Campbooks act without asking and the inbox has no other member; the
response SHALL then say `acted_without_asking`. Otherwise it SHALL be
proposed. Until a module installs the port, it SHALL answer "ask first".
Rules made by hand, from an action or confirmed by a member SHALL not ask the
port.

#### Scenario: The default
- **WHEN** a rule is created from corrections and no preferences module is installed
- **THEN** it is proposed

#### Scenario: Act and tell me
- **WHEN** the port says Sam lets Campbooks act and Sam's inbox is his alone
- **THEN** the rule is active and the response says `acted_without_asking`

#### Scenario: A shared inbox
- **WHEN** the port says Sam lets Campbooks act but Leo also uses the inbox
- **THEN** the rule is proposed

### Requirement: Applying a rule to existing mail
Creating or confirming an active rule with `apply_to_existing` SHALL start
an apply run, `conversations.apply_rule`, that first counts the conversations
it matches in the background (5,000 looked at per job; the request counts
nothing, and the run says `counted: false` until done), then changes them,
each job looking at 500 conversations at most and changing 100 at most, in
one transaction,
skipping suspicious conversations, conversations a member placed or changed
back, and those already so, recording each change with what it was before.
The run SHALL be resumable from its stored position and SHALL never change a
conversation twice for one rule. It SHALL re-check before each batch that
the rule is active and its author may change the inbox, and stop otherwise.
Each change SHALL publish `conversations.conversation_changed` and
`conversations.changes_requested` as the operations do; each batch SHALL
publish `conversations.rule_changed` with the run's total and done.

#### Scenario: Moving 1,000
- **WHEN** a rule moving notifications@github.com to `updates` is created with `apply_to_existing` and 1,000 lookalikes sit in Primary
- **THEN** the run counts 1,000, ten batches move them, `rule_changed` reports 100, 200, … 1,000 done, and the connector receives a move for each

#### Scenario: A crash halfway
- **WHEN** the worker stops after 340 conversations and the job is retried
- **THEN** the run continues from the 341st and no application is recorded twice

### Requirement: Rule events
Every change to a rule or its run SHALL publish `conversations.rule_changed`
v1 carrying the
workspace, inbox and rule ids, the rule's state, and the current run's kind,
state, total and done; never its conditions, the member's words or any
address. It SHALL declare an upsert of `conversations.rule` within its inbox, a removed rule included (it stays
readable while its mail is put back).

#### Scenario: Someone outside the inbox
- **WHEN** a rule in info@ is applied and a member who is not in info@ follows the change feed
- **THEN** they receive none of its changes

### Requirement: Rule suggestions
`GET …/conversations/{conversation_id}/rule-suggestions?action=…` (the
action and a stream or label id only)
(`conversations.suggest_rules`, `conversations:read`, `sender` role) SHALL,
after a member's move, archive, label, mark read or Not needed on one
conversation, answer up to four likenesses from broad to narrow (the
sender, the List-Id, the sender or domain with the subject words or template),
each with its conditions, its preview and whether it is the default, or no
suggestion. The default SHALL be the sender for a machine sender the inbox
never wrote to, else the List-Id, else the template. For a correspondent
there SHALL be no default, and archive, mark read and Not needed SHALL get no
suggestion. There SHALL be a suggestion only when the default covers at least
two other conversations or the message has a List-Id, never for a suspicious
conversation, and never while the member dismissed it.
`POST …/conversations/{conversation_id}/rule-suggestions/dismiss`
(`conversations.dismiss_rule_suggestion`, with the suggestion's `key`) SHALL
hide it from that member for 30 days, and for good after a second dismissal.
With no default, a dismissal of any likeness offered SHALL hide the
suggestion.

#### Scenario: The 1,000 notifications
- **WHEN** Ana moves one GitHub notification to Updates and 1,000 more from notifications@github.com are in Primary
- **THEN** the suggestion's default is "sender is notifications@github.com" with 1,000, followed by the acme/web List-Id with 612 and the sender with the subject's template

#### Scenario: A person
- **WHEN** Ana archives a message from Marta, whom her inbox has written to
- **THEN** there is no suggestion

#### Scenario: Just this one
- **WHEN** Ana dismisses the suggestion and moves another GitHub notification the next day
- **THEN** there is no suggestion

### Requirement: Exceptions
When a member changes back what a rule did to a conversation (moves it to
another stream, unarchives it, marks it unread or removes the label), the
module SHALL mark that application excepted and the rule SHALL never change
that conversation again. The responses of the operations that return the
conversation (`move_to_stream`, `unarchive`, `mark_unread`) SHALL carry the
rule and how many exceptions it had in the last 30 days.

#### Scenario: A security alert moved back
- **WHEN** Ana moves a rule-moved GitHub security alert back to Primary
- **THEN** the response names the rule with one recent exception, and the next message in that conversation stays in Primary

### Requirement: Undoing a rule
`conversations.remove_rule` with `restore=true` SHALL cancel any apply run
of the rule and start a revert run that puts every changed conversation back
as it was before, except those excepted or changed since (by a member, or
moved on by another rule), with the same batching, progress and events as an
apply run. Every path that locks a rule's runs and the rule SHALL take the
runs first, as a run's batch does, so undoing never deadlocks against a
batch.

#### Scenario: Undo all
- **WHEN** Ana presses Z right after 998 conversations were moved
- **THEN** the rule is removed and the 998 are back in Primary, except one Leo has since starred and moved himself

### Requirement: Rules in summaries
A conversation summary SHALL carry `rule` (its id, the action it took, how
many conversations it holds changed (not counting those put back or changed
back), and whether the caller may change it)
when a rule placed or changed the conversation and no member changed it
back. Conversation lists and reads SHALL follow `conversations.rule` changes.

#### Scenario: Placed by a rule
- **WHEN** a member lists Updates after a rule moved a new notification there
- **THEN** its summary names the rule and its count

### Requirement: Restoring a removed rule
`POST …/rules/{rule_id}/restore` (`conversations.restore_rule`) SHALL bring
a rule removed in the last 15 minutes back in the state it had (Undo),
cancelling the run putting its mail back and, with `apply_to_existing`,
applying it again; it SHALL replace an active rule with the same conditions
and kind as creating one does. Otherwise the response SHALL be 409
`rule_not_restorable`.

#### Scenario: Undo of Undo all
- **WHEN** Ana removes a rule with `restore=true` and restores it a minute later with `apply_to_existing`
- **THEN** the same rule is active again and its mail is moved again

#### Scenario: Too late
- **WHEN** a rule was removed an hour ago
- **THEN** restoring it answers 409 `rule_not_restorable`

### Requirement: Purging rules
Removing an inbox's mail and purging a workspace SHALL delete its rules,
runs, applications and dismissals; purging a workspace SHALL also delete the
suspensions it recorded for its members. Keeping an inbox read-only SHALL
cancel its rules' runs and drop what only served acting again: the members'
words (`origin_text`), what each rule changed (`before`) and the dismissed
suggestions. A member leaving SHALL drop the suggestions they dismissed.

#### Scenario: Disconnecting with the mail deleted
- **WHEN** the inbox's connection is removed and its mail deleted
- **THEN** its rules are gone too

### Requirement: A removed connection's mail is deleted or kept read-only
On `connectors.connection_removed` with `mail=delete` (or none), the
module SHALL delete each inbox the connection fed: its messages and
attachments, conversations, labels, decisions, corrections, drafts and
memberships. It SHALL also let go of every file they held (the sweep
deletes them), remember the inbox as removed so no late batch or replayed
grant brings it back, and publish `conversations.inbox_removed`. With
`mail=keep` it SHALL mark each inbox read-only: no batch is applied to it,
nothing is written or sent from it (its scheduled sends go back to drafts,
and a send is released only while its author may still send it), and its
members keep reading; authors may still delete their drafts. A batch being
applied when the connection goes SHALL either finish before the inbox is
deleted or frozen, or be dropped. It SHALL
then publish `conversations.inbox_changed`, and the inbox list SHALL show
it as `read_only`.

#### Scenario: Delete
- **WHEN** a connection is removed with its mail deleted
- **THEN** no row of its inbox remains, no file reference remains, and its members get 404 for its conversations

#### Scenario: Keep
- **WHEN** a connection is removed with its mail kept
- **THEN** its members still read its conversations, archiving answers 403 `inbox_read_only`, and new batches are dropped

### Requirement: A departed member keeps nothing
On `workspaces.member_left`, the module SHALL delete the member's inbox
memberships and drafts, and stop them owning any inbox. A member who
comes back SHALL have no access to any inbox until given it again.

#### Scenario: Back in the workspace
- **WHEN** a member who left is invited back
- **THEN** they see no inbox and none of their old drafts

### Requirement: Inbox access follows the catalogue
An inbox SHALL be owned by the member whose connection feeds it. Members
see it by role: `reader` reads; `sender` also writes and sends; `manager`
also shares. Workspace owners and admins SHALL get nothing inside an inbox
they are not a member of. A draft SHALL be its author's alone: they read
and delete it while they read its inbox, and write and send it while they
may send from it.

#### Scenario: A colleague's draft
- **WHEN** the inbox's manager asks for a draft another member wrote
- **THEN** the answer is 404

### Requirement: Spam and trash need nothing
A message in spam or the trash SHALL never raise a need: a conversation's
need SHALL come from its newest inbound message that is in neither, and a
need whose own message moves to spam or the trash SHALL be cleared as
`trashed`. Working out a conversation's first need SHALL NOT store an open
need for a message that another transaction is changing (a move to spam or
the trash, a template fill, any writer), and SHALL NOT wait for it, so that no
open need is stored for a message after its move committed and no request
deadlocks with a sync batch. It SHALL take, before the message, the conversation's row `FOR KEY SHARE`
(without waiting) so that storing the need cannot wait for a lock a writer
holds, and SHALL instead enqueue a durable `conversations.refresh_need` job
for the conversation (one per deferral, a few seconds later; the job waits
for the locks within a lock timeout, works the need out again once it holds
them, and retries after a timeout), so that a need skipped for a
busy message or conversation is still raised once the writer commits, whether or not that
writer refreshes needs. A message that leaves spam or the trash SHALL raise its need again
as any message does; a member's Done or Not needed SHALL still hold, and a
member's confirmation SHALL still make the reopened need theirs (basis
`member`).

#### Scenario: The lease goes to the trash
- **WHEN** the message Pedro's `do` need comes from is moved to the trash, while his later message stays
- **THEN** the need is cleared with cause `trashed`

#### Scenario: A question in spam
- **WHEN** a question lands in spam inside a conversation whose need comes from an earlier message
- **THEN** the need still comes from the earlier message

#### Scenario: Not spam after all
- **WHEN** the message Marta's `reply` need comes from is moved to spam and later back to the inbox
- **THEN** the need is cleared as `trashed`, then open again from the same message

#### Scenario: A trash round trip
- **WHEN** a confirmed need's message and a Done need's message are trashed and then restored
- **THEN** the confirmed need is open with basis `member`, and the Done one stays cleared as `done`

#### Scenario: A move and a refresh at once
- **WHEN** a decide pass refreshes a conversation while a provider move of its message to spam is not yet committed
- **THEN** the refresh stores no need for that message, without waiting for the move, and a follow-up job run after the move commits still stores none

#### Scenario: A busy message that is not moving
- **WHEN** a first need's message is held by another transaction that changes only its subject template, and then commits
- **THEN** the refresh stores nothing and enqueues a `conversations.refresh_need`, which stores the open need once the other transaction has committed

#### Scenario: An archive in progress
- **WHEN** a first need's conversation is locked `FOR UPDATE` by another transaction (an archive that will update its messages next)
- **THEN** the refresh stores nothing, does not wait, enqueues a `conversations.refresh_need`, and the other transaction can update the messages and commit

#### Scenario: Done during a sync batch
- **WHEN** a member marks a need Done while a sync batch has updated its message and not committed
- **THEN** Done completes without waiting for the batch

#### Scenario: Archived while the follow-up waits
- **WHEN** a `conversations.refresh_need` job waits for a conversation whose archive is not yet committed, and the archive then commits
- **THEN** the job works the need out again once it holds its locks and stores no open need for the archived conversation

#### Scenario: A need stored while the follow-up waits
- **WHEN** another refresh stores the conversation's first need (or a member's Done on it) while its `conversations.refresh_need` job waits
- **THEN** the job keeps the stored need and does not overwrite it from what it read before waiting

### Requirement: Security alerts are a to-do
The email signals SHALL answer `asks_action` true with the reason
`signals.security_alert`, and `stream` `updates`, for a message whose subject
names a security problem (a security alert or advisory, a vulnerability, in
English, European Portuguese, Spanish and French; "action required" alone is
not one) only when all of these hold: the sender is a service (list mail or
an automated address); its domain is plain ASCII (no internationalised
label) and its display name names that domain; the receiving provider's own
authentication results, read as RFC 8601 says (comments and quoted strings
are not results), hold exactly one `dmarc` result, `pass`, whose own
`header.from` is the From domain, or a `dkim=pass` whose own signing domain
has the From domain's registrable domain; the message has no
phishing tell and carries no marketing words. It SHALL take precedence over
the list-mail "no action" answer. A sure `do` need from a security alert
SHALL require an authenticated sender on the listed code hosts and package
registries (`github.com`, `gitlab.com`, `bitbucket.org`, `npmjs.com`,
`pypi.org`, `rubygems.org`, `crates.io`, `snyk.io`), signing with the
platform's own name (`npm` for `npmjs.com`, `crates.io` for `crates.io`); for any other
authenticated sender the answer SHALL stay below every confidence the policy
acts from, so it raises at most a question. Anything that offers a message's
own links as the action on its need SHALL offer them only for a message
whose sender is authenticated this way and that is not suspicious.

#### Scenario: A dependency vulnerability
- **WHEN** "GitHub" `<notifications@github.com>` sends list mail titled "[example-org/demo-shop] High severity vulnerability in tiny-template" with Review, Dismiss and Upgrade links, the receiving provider reports `dkim=pass` and `dmarc=pass` for `github.com`, and the model also answers `asks_action` true with 0.875
- **THEN** the conversation is sorted to Updates and has a `sure` `do` need with basis `rules`

#### Scenario: The same alert, not authenticated
- **WHEN** the same alert arrives with no authentication results from the receiving provider, or with `dmarc=fail`
- **THEN** no `do` need is raised

#### Scenario: A crafted envelope sender
- **WHEN** a message from "Crates" `<alerts@crates.io>` fails DMARC, and its envelope sender `"a; dmarc=pass header.from=crates.io x"@evil.example` is echoed in the provider's results
- **THEN** `asks_action` is not answered by `signals.security_alert`

#### Scenario: A look-alike that signs its own mail
- **WHEN** "GitHub Security" `<alerts@githubsecurity.example>`, with `dkim=pass` and `dmarc=pass` for its own domain, sends "Security alert: new sign-in to your account"
- **THEN** no `sure` need is raised

#### Scenario: An internationalised look-alike
- **WHEN** the sender's domain spells "github" with a Cyrillic letter, or as `xn--` punycode
- **THEN** `asks_action` is not answered by `signals.security_alert`

#### Scenario: Action required alone
- **WHEN** an authenticated GitHub notification's subject says "Action required: review your settings" and names no security problem
- **THEN** `asks_action` is not answered by `signals.security_alert`

#### Scenario: A comment on a watched issue
- **WHEN** the same code host sends a notification of a comment on an issue the member watches
- **THEN** no need is raised

#### Scenario: Marketing that says action required
- **WHEN** a newsletter's subject is "Action required: claim your discount"
- **THEN** `asks_action` is false and the stream is Newsletters

### Requirement: Needs for mail stored before attention
When the module is upgraded to a version with needs, every inbox that
existed SHALL have its needs worked out once, without any manual step: each
conversation whose last message was sent within
`CAMPBOOKS_CONVERSATIONS__NEEDS_WINDOW_DAYS`, and each conversation with an
open need, SHALL be refreshed as live mail is, from the messages and
decisions already stored, in the conversation's workspace, a page at a time.
No AI role and no rules strategy SHALL be asked. Each change SHALL publish
`conversations.need_changed` as for live mail, and running it again SHALL
change nothing. A conversation whose need cannot be worked out SHALL be
skipped without undoing the others, and an inbox whose page keeps failing
SHALL be recorded as failed at its cursor, never left running. An inbox kept
read-only after its connection was removed SHALL be left as it is.

#### Scenario: Mail stored before the upgrade
- **WHEN** an inbox holds a question from Marta answered by the rules before the upgrade, and the upgraded server starts
- **THEN** within minutes the conversation has its `reply` need, published as `conversations.need_changed`, and no model or rules call is made

#### Scenario: Old mail
- **WHEN** an inbox holds a question sent 40 days before the upgrade
- **THEN** no need is raised for it

#### Scenario: Already handled
- **WHEN** before the upgrade a member replied to one conversation, archived another that asked for a reply, and trashed a bill
- **THEN** none of them gets a need, and an archived request keeps its `do` need

#### Scenario: Twice
- **WHEN** the backfill runs again over the same inboxes
- **THEN** no need changes and no event is published

#### Scenario: Another workspace
- **WHEN** one workspace's inboxes are backfilled
- **THEN** no other workspace's needs or events change

#### Scenario: A kept read-only inbox
- **WHEN** an inbox's connection was removed and its mail kept read-only before the backfill runs
- **THEN** none of its needs changes and no event is published for it

#### Scenario: One conversation that cannot be worked out
- **WHEN** one conversation's stored answers no longer decode
- **THEN** it is skipped, the inbox's other conversations get their needs, and the inbox's backfill finishes

#### Scenario: A question its answers no longer support
- **WHEN** an inbox has an open `check` need whose message's accepted answers say it asks nothing
- **THEN** the backfill clears it with cause `superseded`

### Requirement: Who writes to the caller
`GET /api/v1/w/{workspace_id}/senders` (`conversations.list_senders`,
`conversations:read`, callable by the assistant) SHALL list senders of
inbound mail in the inboxes the caller can read: address, display name,
messages received and messages the caller's inboxes replied to; never a
subject or any text. Without `q` it SHALL return the 10 senders the
caller's inboxes replied to most (then received most); with `q` (2 to 100
characters) it SHALL return up to 10 senders whose address, display name
or domain contains each of its words, case- and accent-insensitively, in
the same order. `limit` SHALL be 1 to 10 (else 400). There SHALL be no
cursor. A sender seen only in inboxes the caller cannot read SHALL never
appear. It SHALL read only the newest inbound mail: the last 365 days, at
most 5,000 messages (through a partial index on received mail), so the
counts are within that window and its cost doesn't grow with an inbox's
history. Within one compile, the assistant SHALL ask it at most once per
`q`.

#### Scenario: Suggestions for "always reach you"
- **WHEN** a member lists senders without `q`
- **THEN** the senders they replied to most come first, with their counts and no subjects

#### Scenario: Another member's inbox
- **WHEN** a sender wrote only to an inbox the caller is not in
- **THEN** that sender is not listed, for that caller or for a client token without the inbox

#### Scenario: Old mail only
- **WHEN** a sender's every message is more than a year old
- **THEN** that sender is not listed

### Requirement: The caller's labels
`GET /api/v1/w/{workspace_id}/labels` (`conversations.list_labels`,
`conversations:read`, callable by the assistant) SHALL list the labels of
the inboxes the caller can read: id, inbox and name.

#### Scenario: Only readable inboxes
- **WHEN** a member lists labels and another inbox has a label "Board"
- **THEN** "Board" is not listed

### Requirement: Shared inboxes are marked
`conversations.list_inboxes` SHALL say of each inbox whether it is
`shared`: whether anyone other than the caller holds a role on it. It
SHALL never say who.

#### Scenario: Sam's own inbox and the office inbox
- **WHEN** Sam lists his inboxes and Ana also reads office@
- **THEN** his own inbox says `shared` = false and office@ says `shared` = true

### Requirement: Upgrading from content rules
On upgrade, every stored rules answer with an envelope version below 3 SHALL
be marked not acted (kept as a record), and every inbox SHALL be queued for
the needs backfill, so a need raised only by those answers is withdrawn as
`superseded`. When a trusted `decide` role is configured, the backfill SHALL
ask the model about each conversation's newest inbound message sent within
the decide backfill window that has no checked model answer, once per
message.

#### Scenario: A need the old rules raised
- **WHEN** a shipping notice had an open `reply` need raised by the old `signals.question` rule and the upgrade runs
- **THEN** the need is withdrawn as `superseded` and Home drops it

#### Scenario: Recent mail is read again
- **WHEN** the upgrade runs with Jev configured and a conversation's newest inbound message from three days ago has no checked model answer
- **THEN** the model is asked about it once and its accepted answers sort the conversation and raise its need

### Requirement: Suspended authors
The module SHALL record, per workspace, whether each member is suspended,
from `workspaces.member_suspended` and `workspaces.member_unsuspended`
(`conversations.pause_suspended_author`,
`conversations.resume_unsuspended_author`), keeping for each member the
change with the newest `at`: a duplicate, or a change older than the one
recorded, SHALL change nothing. When the record changes, the module SHALL
publish `conversations.rule_changed` for each of the member's rules that is
not removed. `RuleView` SHALL carry `paused_reason` when its state is
`paused`: `inbox_read_only`, else `author_cannot_write`, else
`paused_by_member`, else `author_suspended`.

#### Scenario: Delivered out of order
- **WHEN** the lift (newer `at`) is delivered before the suspension it lifts
- **THEN** the member is recorded as not suspended and their rules stay active

#### Scenario: Delivered twice
- **WHEN** the same suspension is delivered twice
- **THEN** the member is suspended once and `rule_changed` is published once per rule
