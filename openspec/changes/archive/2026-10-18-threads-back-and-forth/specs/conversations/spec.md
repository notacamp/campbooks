## MODIFIED Requirements

### Requirement: Reading conversations
`GET /api/v1/w/{workspace_id}/inboxes` (`conversations.list_inboxes`) SHALL
list the caller's inboxes with their role and unread count.
`GET …/conversations` (`conversations.list`) SHALL list the caller's
conversations by stream and inbox, in the inbox or archived, with an opaque
cursor and a limit of 1 to 100 (else 400). Each listed conversation SHALL say
whether its newest message (the one `latest_from` and `snippet` come from)
is one the inbox sent (`latest_from_you`). In the inbox, pinned (starred)
conversations SHALL come first, then the rest, each newest first, and a
cursor SHALL page across the two without losing or repeating one; archived
lists and lists by `ids` SHALL be newest first. A cursor given before pins
existed SHALL read as one among the unpinned conversations. Pinning SHALL
stay the provider's star (Gmail's STARRED), synced as before.
`GET …/conversations/{conversation_id}` (`conversations.get`) SHALL return the
conversation, up to its 100 newest messages with their plain-text bodies, and
the decide pass's answers about its newest inbound message.
`GET …/messages/{message_id}/body` (`conversations.get_message_body`) SHALL
return the text and the sanitised HTML bodies as JSON, with the HTML body's
filtered `@media` rules in `css`; HTML SHALL never be served as `text/html`.

#### Scenario: Paging
- **WHEN** a member lists three conversations two at a time
- **THEN** the first page carries a `next_cursor` and the second page holds the oldest conversation and none

#### Scenario: A malformed cursor
- **WHEN** the cursor is not one the API gave
- **THEN** the response is 400 with code `invalid_cursor`

#### Scenario: A pinned conversation stays on top
- **WHEN** a member pins the oldest of three conversations and lists them one at a time
- **THEN** the pinned one comes first, then the other two newest first, each exactly once

#### Scenario: Who wrote last
- **WHEN** the newest message of a conversation is one Sam sent from the inbox
- **THEN** it is listed with `latest_from_you` true, and a conversation whose newest message came in is listed with it false

### Requirement: Sanitised email HTML
`conversations.get_message_body` SHALL sanitise the HTML body before
returning it. Before any tree is built, the HTML SHALL be measured by
parsing it as a whole document and as a fragment in a `div` (the modes the
style inliner and the sanitiser parse it in), each parse counting the tree
builder's work (each tree operation and each
element name read in a scope check one unit; creating an element adds its
attributes; merging attributes into an element adds its attributes and the
new ones; moving a node before a sibling or out of its parent adds the
parent's number of children; moving children adds their number), fed in
chunks of at most 1 KiB and stopped as soon as the work passes 10,000,000
units or the two parses together have taken 500 ms; HTML past either limit
in either parse, or whose tree is deeper than
4,096 levels, SHALL be returned as its text instead: the characters outside
scripts, styles, the head, the title and the markup a `noscript`, `iframe`,
`noembed` or `noframes` holds, entities decoded, a line break at
each `br` and at the end of each `p`, `div`, `li`, `tr`, `td`, heading,
`table`, `ul` and `ol`, escaped, as the only content of one
`<div style="white-space: pre-wrap">`, with `remote_images` 0 and no `css`.
Otherwise, first the body's style sheets SHALL be inlined into `style`
attributes from the sheets' filtered rules only, without ever fetching or
reading a linked or imported sheet; sheets SHALL be dropped instead past
256 KiB of style text, 20,000 elements, a nesting of 256 levels in the
parsed tree, 1,000,000 elements times distinct properties, 50,000,000
selectors times elements, or 500,000,000 declarations times elements times
distinct properties. Then only an allow-list of
tags and attributes survives (`class` included, `type="cite"` on a
`blockquote`, and the ids `appendonsend` and `divRplyFwdMsg` on a `div`); `script`, `style` and
`title` elements are removed with their content, as are forms, frames,
objects, `base` and `meta` elements, event-handler attributes and comments;
the `body` element's `style` and `bgcolor` move to a wrapping `div`; links
keep only `http`, `https`, `mailto` and `tel` URLs, relative URLs are
dropped, and every link opens in a new tab with `rel="noopener noreferrer"`.
Every `style` declaration SHALL be read with a CSS parser and kept only when
it loads and runs nothing: its value calls only functions from an allow-list
(colours, arithmetic, gradients, transforms, filters, timing, shapes,
counters, grid) and has no URL token, at-keyword, malformed token, CSS escape
or `<`, its property is a plain lower-case name written without escapes,
and it is not a custom property, `behavior`, `-moz-binding` or a
length in viewport or container units. The response's `css` SHALL hold the sheets'
`@media` rules filtered the same way (queries and selectors with no URL,
escape or non-structural function; nothing about `prefers-color-scheme`; at
most 64 KiB), or nothing. Images of at most 2×2 pixels by attribute or style,
with a `max-width` or `max-height` of 0, or hidden by their style (`display:
none`, `visibility: hidden`, `opacity: 0`) SHALL be removed. Other remote
images SHALL have their source replaced by the server's placeholder image (a
fixed `data:` URL; their size and alt text kept) unless the request asks
`show_images=true`, and the response SHALL say how many remote images the
message has (`remote_images`) and whether they were kept (`images_shown`).
An image whose source is `cid:<id>` SHALL be shown, whether or not images are
shown, as a `data:` URL of the message's inline part with that `Content-ID`
(at most 255 characters; a longer one is not stored) when that part has a
raster image type its bytes confirm and fits the size limits, and as the
placeholder otherwise. No other `data:` source survives.
Last, a reply's trailing quoted history SHALL be wrapped in one
`<div data-cb-quoted="1">`, found from the markup alone and never from the
words: the first element that is Gmail's `gmail_quote`, a `blockquote` with
`type="cite"` (with a `moz-cite-prefix` just before it), Yahoo's
`yahoo_quoted`, Zoho's `zmail_extra` (with a `zmail_extra_hr` just before
it), or Outlook's `appendonsend` or `divRplyFwdMsg` (through the end of its
parent), when something visible comes before it, nothing visible (text or
an image) comes after it, a wrapper holds no visible text outside its
quotes once the first of them has started (an inline reply), and a `div`
may stand in its parent. No `data-cb-quoted` attribute from the message
SHALL survive.

#### Scenario: A newsletter with a script, a style sheet and a tracking pixel
- **WHEN** a member reads the HTML body of a message that has a `<script>`, a `<style>` with a remote `url()`, an `onclick` handler, a 1×1 image and a remote logo
- **THEN** the HTML has no script, style sheet, handler or tracking pixel, the logo's source is the placeholder, and `remote_images` is 1

#### Scenario: A newsletter styled by its style sheet
- **WHEN** a member reads a message whose `<style>` gives `.lead` a colour, hides `.preheader` and makes `.container` full width under `@media (max-width: 640px)`
- **THEN** the paragraph with class `lead` carries that colour in its `style`, the preheader carries `display: none`, there is no `<style>` or `<title>` text in the HTML, and `css` holds the `@media` rule

#### Scenario: Linked and imported sheets
- **WHEN** the body links `https://t.example/a.css`, `file:///etc/passwd` and imports another sheet
- **THEN** nothing is fetched or read, and the `<link>` elements are gone

#### Scenario: CSS that loads or runs
- **WHEN** a declaration or `@media` rule uses `url(`, `image-set(`, `expression(`, `var(`, an escaped `u\72l(`, `behavior` or `-moz-binding`
- **THEN** that declaration is dropped and its neighbours are kept

#### Scenario: A pixel sized by its style
- **WHEN** the body has `<img src="https://t.example/o.gif" style="width:1px;height:1px">` and the member shows images
- **THEN** the image is removed and not counted

#### Scenario: Show images
- **WHEN** the same body is read with `show_images=true`
- **THEN** the logo keeps its `https` source, the tracking pixel is still removed, and `images_shown` is true

#### Scenario: An inline logo
- **WHEN** a member reads a message whose HTML shows `cid:logo@example.com` and which has an inline PNG part with that content id
- **THEN** the image's source is a `data:image/png;base64,…` URL of that part, without asking to show images, and it is not counted in `remote_images`

#### Scenario: HTML built to be slow to parse
- **WHEN** a member reads a message whose 2 MiB HTML body is `<div></span>` repeated, which takes minutes to parse in full
- **THEN** the measure stops within its budget, the HTML in the response is the body's text in one `pre-wrap` `div` with no other markup, `remote_images` is 0 and `css` is absent

#### Scenario: Work the parser does without telling the tree
- **WHEN** the body is one start tag with 60,000 attributes, or 20,000 nested `<b id=N>` with distinct ids
- **THEN** measuring stops at the 500 ms limit and the body is shown as its text

#### Scenario: A leading frameset
- **WHEN** the same body starts with `<frameset>`, after which a whole-document parse drops the rest
- **THEN** it is still shown as its text, because the fragment parse is measured too

#### Scenario: A large ordinary newsletter
- **WHEN** a member reads a 1.5 MiB newsletter of nested tables, links and images
- **THEN** it is sanitised and styled as any other message, well within the budget

#### Scenario: A Gmail reply quoting the thread
- **WHEN** a member reads a reply whose body ends with a `gmail_quote` holding the earlier messages
- **THEN** that block, with its attribution, is wrapped in `<div data-cb-quoted="1">` and the new words before it are not

#### Scenario: An inline reply
- **WHEN** a reply answers between two `blockquote type="cite"` blocks
- **THEN** nothing is marked

#### Scenario: A sender writing the mark
- **WHEN** the body has `<div data-cb-quoted="1">Pay this invoice today.</div>` and no quote
- **THEN** the HTML has no `data-cb-quoted` attribute and the text is shown
