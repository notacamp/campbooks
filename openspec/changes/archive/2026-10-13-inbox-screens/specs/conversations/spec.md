## MODIFIED Requirements

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

## ADDED Requirements

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
