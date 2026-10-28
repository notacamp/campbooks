# inbox-ui Specification

## Purpose
The inbox screens of the app: a workspace's streams, each stream's
conversations, and the reader with the thread, the assistant's suggestions
and their reasons, safe email bodies, tidying with undo, and the slot the
send slice's composer fills.

## Requirements

### Requirement: Streams, not people
The inbox SHALL be its streams: Primary, Updates, Receipts and Newsletters,
listed under Inbox in the sidebar (chips on phones) with their unread counts,
opened with G then 1 to 4. A stream's page SHALL list its pinned
conversations first, then the rest newest first, a page at a time, with
sender, time, subject, snippet, unread, pinned and message count, and All or
Unread. Six places SHALL sit under the streams, after a quiet divider, in this
order: Sent, Pinned, Drafts, Archive, Spam and Trash, each with its icon and
no count. Sent, Archive, Spam and Trash SHALL open with G then 5 to 8, Pinned
with G then 9 and Drafts with G then 0: the keys follow history, the order
follows meaning. On phones they SHALL follow the stream chips, after a
divider, in the same row, which scrolls sideways on its own; the page never
does. Sent, Pinned, Archive, Spam and Trash are views; Drafts lists your
saved drafts and not conversations. None is a stream: nothing is sorted into
them, and a conversation can be in a stream and a view at once. Pinned SHALL
list the conversations the user pinned, in any stream, archived or not,
outside the trash, newest first; pinning or unpinning SHALL add it to or take
it out of the open Pinned list at once. Any other place in the address SHALL
open Primary.

#### Scenario: Open a stream from the keyboard
- **WHEN** the user presses G then 3
- **THEN** the Receipts stream opens

#### Scenario: A pinned conversation
- **WHEN** the user pins an older conversation in Primary
- **THEN** it moves to the top of Primary, marked pinned, and stays there as newer mail arrives

#### Scenario: Open a view from the keyboard
- **WHEN** the user presses G then 8
- **THEN** Trash opens, marked as the current place in the sidebar

#### Scenario: Open Pinned and Drafts from the keyboard
- **WHEN** the user presses G then 9, then G then 0
- **THEN** Pinned opens, then Drafts, each marked as the current place in the sidebar, and G then 5 to 8 still open Sent, Archive, Spam and Trash

#### Scenario: Pinned across streams
- **WHEN** the user has pinned a conversation in Primary and an archived one in Receipts
- **THEN** Pinned lists both, newest first, and unpinning one takes it out of the list at once

#### Scenario: Views on a phone
- **WHEN** the inbox is open at 375px
- **THEN** the chips show the four streams, a divider, then Sent, Pinned, Drafts, Archive, Spam and Trash, and only the chip row scrolls sideways

#### Scenario: An empty view
- **WHEN** Spam has nothing in it
- **THEN** it says "Nothing in Spam." in one line

### Requirement: Suggestions shown as suggestions
The decide pass's answers SHALL be shown as suggestions, never applied by the
client on its own: a row whose conversation has a stream suggestion SHALL say
"Suggests <stream>"; the reader's note SHALL say what the decide pass thinks
in plain words, as suggestions, and who placed the conversation, with no
percentage. "How Campbooks decided" (closed by default) SHALL list, for each
question, the answer in words, the stored reasons in the user's language, the
source (rules or model), the confidence and the threshold the answer was held
to, and whether it acted. A stream suggestion SHALL offer "Move to <stream>"
(1), which the user can undo. The note SHALL offer Reply, opening the same
reply dialog as the reader's own Reply, whenever it says a reply is needed (a
needs_reply suggestion or an open Reply need) and the user can reply. The
decide pass's answers are about the conversation's newest inbound message:
when the conversation has no open need and the user sent a message after
that one, the note SHALL say they replied and when ("You replied on 23 Oct,
so nothing here waits on you."), SHALL NOT suggest a reply, an ask, a date
or urgency from those answers, and SHALL offer no Reply; a suspicious
warning SHALL still be said.

#### Scenario: Taking a suggestion
- **WHEN** the user opens a newsletter that the rules suggest for Newsletters and presses 1
- **THEN** it moves to Newsletters, the toast offers Undo, and "How Campbooks decided" had listed the mailing-list header reason

#### Scenario: Replying from the note
- **WHEN** the user opens a conversation whose note says it needs a reply
- **THEN** the note offers Reply, which opens Scout's reply dialog

#### Scenario: No numbers up front
- **WHEN** the user opens a conversation whose stream answer has a confidence of 0.87
- **THEN** the note shows no percentage, and 0.87 appears only inside "How Campbooks decided"

#### Scenario: After the user replied
- **WHEN** the user opens a conversation with no open need whose newest inbound message the decide pass read as needing a reply, and they sent a reply on 23 Oct after it
- **THEN** the note says "You replied on 23 Oct, so nothing here waits on you.", does not say it needs a reply, and offers no Reply

#### Scenario: A warning stands after a reply
- **WHEN** the user replied after a message the decide pass found suspicious
- **THEN** the note says they replied and still says the message looks suspicious

### Requirement: Reading a conversation
The reader SHALL show the conversation's subject, the note, and its messages
newest first with the newest (and any unread) open and older ones folded to
one line until opened, each with sender, recipients, time and attachments.
A message the inbox sent SHALL read "You", then "→" and its first recipient
(and how many others), set in from the left on the sunken surface (further
from 48rem); everyone else's messages SHALL stay flush left. In a thread of
more than four messages the reader SHALL keep the newest two, the first,
every unread one and every one the member opened, and fold each run of two
or more of the others into one "N more messages" button that shows them, as
folded lines, in its place until another conversation opens. A message's
trailing quoted history SHALL be folded behind "Show quoted text" ("Hide
quoted text" once shown): in an HTML body the block the server marked, as a
`details` element that opens with no script, the frame growing to fit; in a
plain-text body the trailing run of lines that start with `>`, when the
message has words of its own before it. Nothing about the words decides what
is quoted.
HTML bodies SHALL be shown only in a sandboxed frame that never runs scripts
(sandbox exactly `allow-same-origin allow-popups
allow-popups-to-escape-sandbox`: no `allow-scripts`, no forms, no top
navigation), with its own Content-Security-Policy and no referrer. The frame
SHALL be as tall as its content (measured, and again when its width or
content changes; never scrolling inside except sideways), SHALL shrink a
layout wider than the reader to fit, down to half size, and SHALL apply the
message's filtered `@media` rules to its own document only, never to the
app's page. Remote images SHALL stay blocked until the user chooses "Show
images" for that message (Shift+I for all). Opening an unread conversation
SHALL mark it read.

#### Scenario: A newsletter with remote images
- **WHEN** the user opens a message whose body has a remote image
- **THEN** the frame's policy allows only `data:` images, the reader says one remote image is hidden, and after "Show images" the frame allows `https` images

#### Scenario: A long message
- **WHEN** the user opens a message whose content is 3,000px tall
- **THEN** the frame is 3,000px tall, cut nowhere and not padded, and the reader scrolls, not the frame

#### Scenario: A fixed 600px layout on a phone
- **WHEN** the user opens a message laid out at 600px wide at 375px
- **THEN** it is shown shrunk to the frame's width and the page does not scroll sideways

#### Scenario: A responsive newsletter on a phone
- **WHEN** a message's `@media (max-width: 620px)` rule makes its container full width and the frame is 343px wide
- **THEN** the rule applies inside the frame, the layout fits without shrinking, and the app's own page is unaffected

#### Scenario: A long back and forth
- **WHEN** the member opens a thread of seven read messages, three of them sent by Sam
- **THEN** the newest two and the first show, "4 more messages" stands between them, Sam's read "You → Marta Silva" set in from the left, and pressing it shows the four as folded lines

#### Scenario: A reply with the whole thread quoted
- **WHEN** the member opens Marta's Gmail reply whose body ends with the earlier messages in a `gmail_quote`
- **THEN** only her new words show, with "Show quoted text" under them, and opening it shows the quote in the frame

#### Scenario: An inline reply is not folded
- **WHEN** a plain-text reply answers between the lines it quotes
- **THEN** the whole message shows and there is no "Show quoted text"

### Requirement: Tidying with undo
Done (E, archive), Pin or unpin (P, the provider's star), read or unread (U)
and move to a stream (M, then 1 to 4) SHALL act on the open conversation, or
on the list's cursor (J/K), show at once, and push an entry that Undo (Z, or
the toast) takes back. After Done the next conversation opens (the list, on
phones). The UI SHALL call it Pin everywhere, in en, pt-PT, es-ES and fr.

#### Scenario: Done and Undo
- **WHEN** the user presses E on an open conversation, then Z
- **THEN** the conversation is archived, the toast says "Done · <subject>", and Z unarchives it

#### Scenario: Pin
- **WHEN** the user presses P on an open conversation
- **THEN** it is pinned, the toast says "Pinned · <subject>" with Undo, and Gmail shows it starred

### Requirement: The reply slot
The reader SHALL expose a slot for the send slice's composer, pinned to the
bottom of the reader after the thread and never between or after its
messages in the flow, with Reply (R), Reply all (Shift+R) and Forward (F) that
tell the slot which message to answer and how; without a composer those
actions SHALL be hidden. From 48rem Reply, Reply all and Forward SHALL sit
together in the reader's header right after Done, in that order, then Move;
every action after Done SHALL be an icon button whose tooltip names it and
its key. Delete, Report spam, Pin, Read or Unread, and Previous and Next at
the end, SHALL show as icons while the header has room for them, and
otherwise fold into one "More actions" menu (each row with its name and
key, ↑/↓ between rows, Esc closes), so the header is one row at every
width; their keys work either way. The
pinned area SHALL hold only the open composer: it stays in view while the
thread scrolls under it, growing upward to 60% of the window's height and
then scrolling inside itself; while the composer is closed nothing SHALL be
pinned there. Below 48rem the floating bar above the tab bar SHALL offer
Reply and Done, and an open composer SHALL take its place. Reply SHALL first
open the reply dialog (Scout's likely replies and draft); its "Write my own"
and "Open in composer" SHALL open the slot's composer on the newest message,
the latter on the draft Scout wrote. Reply all and Forward SHALL open the
composer directly.

#### Scenario: No composer yet
- **WHEN** the inbox runs without a reply-slot renderer
- **THEN** the reader shows no Reply, Reply all or Forward

#### Scenario: Write my own
- **WHEN** the user presses R in the reader, then chooses "Write my own"
- **THEN** the dialog closes and the composer opens in the pinned area at the bottom of the reader

#### Scenario: Reply beside Reply all and Forward
- **WHEN** the member opens a conversation at 1280px
- **THEN** the reader's header is one row: Done, then Reply (R), Reply all (Shift+R) and Forward (F) side by side as icons whose tooltips name them and their keys, and no "Reply to <name>" button sits at the bottom of the reader

#### Scenario: A narrow reader folds the rest into More
- **WHEN** the member opens a conversation at 1100px wide, where the reader is too narrow for every icon
- **THEN** the header is still one row: Done, Reply, Reply all, Forward and Move, then "More actions" holding Delete, Report spam, Pin, Mark unread, Previous and Next with their keys, and # still deletes

#### Scenario: A long thread on a desktop
- **WHEN** the member opens a twelve-message thread at 1440px and scrolls it
- **THEN** Reply stays in the header beside Reply all and Forward, nothing is pinned at the bottom, and no reply box sits between or after the messages

### Requirement: Separate screens on phones
From 64rem the list and the reader SHALL show side by side at every width,
the reader taking the width the sidebar and the list leave and never wrapping
under the list, and the page SHALL be exactly the window's height: the
sidebar, the list and the reader each scroll on their own, a pane's header
staying at the top of its pane; below 64rem they SHALL be separate screens, the reader with
"Back to <stream>" (Esc), and below 48rem the reader's Done (and Reply) SHALL
float above the tab bar. Every screen SHALL exist in en, pt-PT, es-ES and fr,
show loading, empty and error states (including no mailbox and a conversation
that is gone), fit 375px without horizontal scrolling and pass axe in light
and dark. No content (a long sender address, subject, URL or word) SHALL make
a screen wider than the viewport at any width; long unbroken text wraps.

#### Scenario: Opening a conversation on a phone
- **WHEN** the user taps a conversation at 375px
- **THEN** the reader replaces the list, and Back returns to the stream

#### Scenario: A narrow desktop window
- **WHEN** a conversation is open at 1100px wide
- **THEN** the reader sits beside the list, its top level with the list's, and the page is no wider than the window

#### Scenario: Long unbroken text
- **WHEN** an open conversation's subject and sender have no spaces or hyphens and are longer than the reader is wide
- **THEN** the text wraps inside the reader and the page does not scroll sideways, at 375px, 1100px and 1440px

#### Scenario: A full inbox on desktop
- **WHEN** a stream holds more conversations than fit in a 1100px or 1440px window and one is open
- **THEN** the page is no taller than the window, the list scrolls inside its pane with its tabs kept in view, and the reader stays in view beside it

### Requirement: Attachments
Each open message SHALL list its attachments, except inline images shown in
its body, with a type icon, the file name (bidirectional and control
characters removed), the size, a Download action and, for raster images and
PDFs, a Preview action. An image preview SHALL show the picture as an image
element of a `blob:` URL whose type the client sets from its raster list,
in a dialog with Download and Close; a PDF preview SHALL open the
attachment in a new tab, served inline under a sandbox policy. HTML, SVG and
every other type SHALL be download-only. V SHALL move focus through the
open message's attachments, wrapping, and Enter SHALL open the focused one;
the list SHALL show the V hint.

#### Scenario: Preview a photo
- **WHEN** the member presses V on a message with a JPEG attachment, then Enter
- **THEN** a dialog shows the photo with its name, Download and Close, and Escape closes it

#### Scenario: An HTML attachment
- **WHEN** a message has an `invoice.html` attachment
- **THEN** its row offers Download only

### Requirement: Placeholder for blocked images
A remote image held back in an email body SHALL show the design system's
placeholder at its declared size instead of the browser's broken-image
icon, and SHALL load nothing until the member chooses "Show images".

#### Scenario: A newsletter banner
- **WHEN** the member opens a newsletter with a remote banner and has not shown images
- **THEN** the banner's place shows the placeholder, and no request leaves for the banner's host

### Requirement: Needs on rows and in the reader
A conversation row with an open need SHALL show a chip for it ("Needs a
reply", "To do", "Pay", "Looks suspicious", or "Check" for a question) and,
when it has one, its due date ("Due Thu", "Overdue"). The reader's note SHALL
lead with the need's sentence, in the user's language, naming the sender (by
address for a `review` need), then the sender's sentence quoted as plain text
with who wrote it and when, then the chips, then Reply (for a `reply` need), Done and Not needed (X)
(E stays the inbox's Done, which archives); a `check` need SHALL ask its
question with Yes (1) and No (2). Every action
SHALL push an entry Undo (Z) takes back. The quote SHALL never be rendered as
HTML or turned into links.

#### Scenario: A question with a date
- **WHEN** the user opens Marta's conversation whose `reply` need is due Wednesday
- **THEN** the note says "Marta Ferreira is waiting for your reply", quotes "Could you confirm Thursday's walkthrough? The builders need to know by Wednesday.", shows "Needs a reply" and "Due Wed", and its Done clears it with Undo in the toast

#### Scenario: Phishing
- **WHEN** the user opens a conversation with a `review` need for suspicious mail
- **THEN** the note says "This may be phishing" with the sender's address and its reasons before the quote, and offers "It's safe"

### Requirement: Moving through a stream with the arrow keys
In a stream's list, ↓ and ↑ SHALL move the keyboard cursor like J and K
(the first press lands on the first row) without opening the conversation,
and Enter SHALL open the row under the cursor. Moving past the last loaded row
SHALL load the next page, and the row the keyboard moves to SHALL be scrolled
into view. In an open conversation the arrow keys SHALL keep their browser
behaviour (scrolling it) while J and K step between conversations. Arrow keys
SHALL NOT be taken from a field, a radio group, a menu, a toolbar, a listbox,
a grid or an open dialog. The key list of the empty reader and the shortcut
sheet SHALL show both bindings ("J or ↓").

#### Scenario: Arrow down, then Enter
- **WHEN** the user presses ↓ three times, ↑ once, ↓ once and Enter in a stream
- **THEN** the cursor moves Q4, NDA, Ledger, NDA, Ledger and the Ledger conversation opens

#### Scenario: Arrows in an open conversation
- **WHEN** a conversation is open and the user presses ↓
- **THEN** the same conversation stays open and the browser scrolls it

#### Scenario: Arrows in a menu
- **WHEN** focus is in a menu, a field or a dialog and the user presses ↓
- **THEN** the list cursor does not move and the control handles the key

### Requirement: Offering to do the same to lookalikes
After the server confirms that the user moved (M), archived (E), marked read
(U, on an unread conversation) or marked Not needed (X, not a warning) one
conversation in the inbox, the inbox SHALL ask `conversations.suggest_rules`
and, when there is a suggestion, show an offer docked above the toasts
(across the bottom below 48rem) that does not block the list: the Visor, the
question in plain words ("Move all 1,000 like this to Updates, and new ones
too?"), the likeness choices with their counts when there are several, up to
three sample subjects with their dates and how many more, whether any are
left alone because they look suspicious, "For everyone in <inbox>" in a
shared inbox, accept (1) and "Just this one" (Esc). "Just this one" SHALL call
`conversations.dismiss_rule_suggestion`. A later action SHALL replace a
pending offer.

#### Scenario: Teaching once
- **WHEN** the user moves a newsletter to Newsletters and 40 more like it are in the inbox
- **THEN** the toast says "Moved to Newsletters · <subject>" and the offer asks "Move all 40 like this to Newsletters, and new ones too?" with its samples

#### Scenario: Just this one
- **WHEN** the user chooses "Just this one"
- **THEN** the offer closes and the suggestion is turned down for 30 days

### Requirement: Progress and Undo for a rule
Accepting SHALL call `conversations.create_rule` with the chosen likeness and
`apply_to_existing`, push an undo entry (Z) that removes the rule and puts
its mail back, and turn the offer into the run's progress, read from
`conversations.get_rule` and kept current by the change feed: "Finding all
the mail like this" while it counts, then "Moving 1,000 to Updates" with a
progress bar and "340 of 1,000 done", then "Moved 998 to Updates. New ones
will go there too." with how many were left alone as suspicious. Undo all
SHALL call `conversations.remove_rule` with `restore=true` and follow the run
putting the mail back. Close (Esc) SHALL hide it; the run goes on.

#### Scenario: Undo all
- **WHEN** the user accepts, sees "Moved 40 to Newsletters", then chooses Undo all
- **THEN** the rule is removed with its mail put back

### Requirement: Mail a rule handled says so
A conversation row a rule placed or changed SHALL show, in the slot
suggestions use, the Visor and "Campbooks moved this · like <n> others
<likeness>" (or archived, marked read, cleared). The reader's note SHALL say
the same and offer "Stop doing this", which removes the rule without putting
mail back and pushes an undo entry that makes it again.

#### Scenario: A week later
- **WHEN** the user opens a notification a rule moved to Updates
- **THEN** the note says "Campbooks moved this · like 1,012 others from notifications@github.com" and offers Stop doing this

### Requirement: The exception question
When an operation's response names a rule with two or more exceptions in the
last 30 days, the inbox SHALL ask in an offer "You changed back 2 this month.
Keep the rule “<sentence>”?" with Remove the rule (1, when the user may change
it) and Keep it (Esc).

#### Scenario: Twice in a month
- **WHEN** the user moves a second rule-moved notification back to Primary within 30 days
- **THEN** the offer asks whether to keep the rule

### Requirement: Kept inboxes are read as history
An inbox `conversations.list_inboxes` lists as `read_only` (its mailbox was
disconnected and its mail kept), or one the person only reads, SHALL open in
the reader with no action that writes: no Done, Pin, read or unread, move,
reply, reply all or forward, no assistant choices, and opening it SHALL NOT
mark it read. Until the inbox is known (its list loading or failed), the
reader SHALL offer no write action either. A kept inbox SHALL say so ("Kept as history", naming its
address). New messages SHALL never offer a read-only inbox to send from.

#### Scenario: A kept conversation
- **WHEN** the person opens a conversation of an inbox listed with `read_only: true`
- **THEN** "Kept as history" is shown, no write action is offered, E does nothing, and nothing is marked read

### Requirement: A bill in the reader
When the open conversation's need is a Pay question (trust `check`, question
`money`), the reader SHALL say that Campbooks can't see the bank and offer
Already paid (1) and Still to pay (2) in place of Yes and No, plus Not needed
(X), each acting as on Home, with Undo. A bill the member said is still to
pay SHALL offer Mark paid (recorded as Already paid) in place of Done.

#### Scenario: Still to pay from the reader
- **WHEN** Sam opens Acme Hosting's invoice and presses 2
- **THEN** the need becomes a sure Pay need and the reader offers Mark paid and Not needed

### Requirement: Links in mail that may be phishing
In a conversation whose need is a warning (`suspicious`), the reader SHALL
show each message body with links that lead nowhere: once the body has
loaded, each link SHALL become plain text (its address kept aside, not as a
link, so it cannot be clicked, focused or opened from the browser's menu,
whatever the mail's styles say), and the frame SHALL NOT allow popups, also
while a frame shown before the warning arrived is replaced. Every other
protection of the body stays as it is.

#### Scenario: A phishing message in the reader
- **WHEN** the user opens a conversation Campbooks flagged as possible phishing and clicks a link in it
- **THEN** nothing opens

### Requirement: Plain-text mail keeps its line breaks
A message shown as plain text SHALL keep its own line breaks and paragraphs,
in the reader and wherever the message is shown (Home's opened item), and a
long word or link SHALL wrap instead of widening the page.

#### Scenario: A plain-text letter
- **WHEN** the user opens a plain-text email written as three short paragraphs
- **THEN** it shows as three paragraphs, not one run-on line

### Requirement: Always show images from a sender
Where a message's remote images are held back and the member may teach rules
in its inbox, the reader SHALL offer, beside Show images, "Always show images
from <sender>" (⇧A, the newest message from someone else). It SHALL open the
smart-rule confirmation: the rule as a sentence with the Smart rule tag,
that it runs on its own with no AI and lives in Scout's memory, a choice
between the sender's address and anyone at its domain (no domain for a free
mail provider), the address choice saying plainly that it holds when the
domain's checks pass and that those checks vouch for the domain, not for the
one person (they authenticate the domain, never the part before the @), how
many conversations it opens up, that it holds only for
mail passing the sender's checks and that tracking pixels stay removed, and
no offer to apply it to the mail already here. Saving SHALL show its result
and offer Undo. A message whose images a rule trusts SHALL say so ("Images
shown: you trust <sender>") instead of offering Show images. Removing the rule
in Scout's memory SHALL say, as it asks to confirm, that the images are held
back again; once removed, Show images is back. A rule created, paused
or removed anywhere (Scout's memory, the assistant, another device, another
member of a shared inbox) SHALL show or hold back the images of the message
bodies the client holds without a reload, and a body a rule trusted SHALL
never load its remote images again from the copy the client kept before the
change. On the web, every change of a rule (made here, its live change, or
the change feed's reset) SHALL drop the kept bodies a rule trusted, so an
open one is read again behind its skeleton and a folded message, or a
conversation opened again, waits for a fresh read; it SHALL re-read in the
background the other kept bodies whose images are held back or offered. On
the desktop and mobile apps, until they follow the change feed live, a rule
changed in the app SHALL do the same; for a change made elsewhere, a body a
rule trusted SHALL be read again every time it is shown, every minute while
it is open and when the window regains focus, and the reader SHALL show the
kept body with its remote images held back (their sources taken out, and the
frame's policy blocking them) until the read made as it opened answers; a
body whose images are held back or offered SHALL be read again after a minute
or when the window regains focus. A member who only reads the inbox SHALL
not be offered it, nor SHALL anyone on mail that may be phishing. No visor:
no AI is involved.

#### Scenario: Trust the Ledger
- **WHEN** the member opens the Weekly Ledger's newsletter, presses Always show images from The Weekly Ledger, picks anyone at weekly-ledger.example and saves
- **THEN** a `show_images` rule for the domain is created, not applied to existing mail, and the reader shows the images with "Images shown: you trust The Weekly Ledger"

#### Scenario: Only reading
- **WHEN** the member's role in the inbox is reader
- **THEN** Show images is offered and Always show images is not

#### Scenario: What trusting an address means
- **WHEN** the member presses Always show images from ana@acme.example
- **THEN** the address choice says the images show for mail from that address when its domain's checks pass, and that those checks vouch for the domain, not for Ana alone

#### Scenario: Removed on another device
- **WHEN** a rule trusting The Weekly Ledger's images is removed on another device, or by another member of the shared inbox, while its newsletter is open here on the web
- **THEN** the open newsletter holds its images back again and offers Show images, without a reload

#### Scenario: Opened again after the rule is removed
- **WHEN** the member read the Weekly Ledger's newsletter on the web while a rule trusted its sender, folded it or left the conversation, and the rule is removed (here, on another device, or by another member)
- **THEN** opening it again shows its skeleton until it is read again, never its kept copy, and it then holds its images back and offers Show images

#### Scenario: Removed while the desktop app is open
- **WHEN** the same rule is removed on another device while the newsletter is open in the desktop app
- **THEN** within a minute, or as soon as the window regains focus, it holds its images back again and offers Show images

#### Scenario: Opened again in the desktop app
- **WHEN** the rule was removed on another device and the member opens the newsletter the desktop app kept
- **THEN** the kept newsletter shows at once with its images held back and no remote image source, and once it is read again its images stay held back and Show images is offered; had the rule still held, they would show once the read answered

### Requirement: HTML mail on the desktop app
The desktop app SHALL render HTML mail as the web app does: the same
sandboxed frame, its own policy and the same sanitised body, styled and with
its `data:` and inline images, remote images held back until Show images or a
trust rule. The desktop page's policy SHALL add only `style-src-attr
'unsafe-inline'` and `img-src 'self' data: blob: https:` to `default-src
'self'`, keep `script-src 'self'` and `style-src 'self'` (never
`'unsafe-inline'` on either), `object-src 'none'`, `base-uri 'none'`,
`form-action 'none'` and `frame-ancestors 'none'`, and leave frames under
`default-src 'self'`; plain `http:` images stay blocked on desktop. Nothing
in the mail frame SHALL run, so Tauri's IPC is out of its reach. A link that
asks for a new window SHALL never open a webview, and the main webview SHALL
never leave the app's own pages: `tauri://localhost`, and
`http(s)://tauri.localhost` only on Windows and Android, where the app is
served from it (elsewhere it is the machine itself), plus the dev server in a
debug build only. A web link opens in the system's browser, a mail link in
the mail app with only its address, copies, subject and body and no
fragment, each value decoded and encoded again with everything but
unreserved characters as `%XX` (a space as `%20`, never `+`; a plus as `%2B`;
a line break in a body, `=`, `/` or `#` encoded too, so no field hides in
a value), and only when it has no authority (`mailto://…`) and its address
part and its `to`, `cc` and `bcc` fields are nothing but addresses (none, or
comma-separated `local@domain`s, valid UTF-8, not starting with `/`, with no
control character, Unicode blank, invisible character such as a bidi
override, a zero-width space or a variation selector, or delimiter such as
CR, LF, `&`, `;`, `?` or `\`, even encoded; other Unicode, as in an
internationalised address, is kept); a subject or body holding `&`, `;` or `?`
once decoded, or still holding a `%XX` escape (encoded twice), SHALL be
dropped, since a mail app that decodes before it splits (on `;` too, as some
older parsers do), or decodes again, would read another field there, and so SHALL a subject that is not valid
UTF-8 or holds a control character (CR and LF included), a line or paragraph
separator (U+2028, U+2029) or an invisible character, and a body that is not
valid UTF-8 or holds a control character other than CR, LF and tab, since a
mail app that decodes first would start another header there; anything else nowhere; no remote URL SHALL be granted
IPC. CI SHALL check the frame in WebKit under the desktop and web policies
read from their definitions, with the app's own sandbox and referrer policy,
and a control proving the check would see a script run. A separate CI job
SHALL drive a debug build of the real shell (tauri-driver, WebKitWebDriver,
a virtual display) against a mock API serving hostile HTML mail, and SHALL
check in the mail frame's realm that no Tauri IPC object exists (while the
app's page has one), that none of the mail's scripts ran and no invoke
reached the API, that inline styles apply and nothing remote loads, that a
`target=_blank` link opens no webview but goes to the system opener alone,
and that a meta refresh or a nested iframe never reaches the system opener.
Only a debug build SHALL record the links it hands to the system opener
(`CAMPBOOKS_DEV_OPEN_OUTSIDE_LOG`) instead of opening them.

#### Scenario: A newsletter on desktop
- **WHEN** the member opens the #160 newsletter on the desktop app
- **THEN** it shows its inline styles and placeholders, nothing remote loads, and after Show images its https images load with no tracking pixel

#### Scenario: Hostile markup
- **WHEN** a body carries a script, an `onerror`, an `<svg onload>`, a `javascript:` link, a form, a meta refresh, `@import` or a remote style sheet
- **THEN** nothing runs, posts, navigates or loads under the desktop policy

#### Scenario: A link in a mail
- **WHEN** the member clicks a web link in a mail on the desktop app
- **THEN** it opens in the system browser, and no Campbooks window shows the remote page

#### Scenario: A look-alike on the machine
- **WHEN** a link to `http://tauri.localhost/` reaches the main window on macOS or Linux
- **THEN** the window stays where it is and the link opens in the system browser

#### Scenario: A mail link with spaces
- **WHEN** the member clicks `mailto:rita@example.com?subject=Tax%20dates&attach=/etc/passwd`
- **THEN** the mail app gets `mailto:rita@example.com?subject=Tax%20dates`

#### Scenario: A field hidden in a value or the fragment
- **WHEN** the member clicks `mailto:rita@example.com?subject=hi#&attach=/etc/passwd` or `mailto:rita@example.com?subject=x;attach=/etc/passwd`
- **THEN** the mail app gets `mailto:rita@example.com?subject=hi`, or `mailto:rita@example.com` (that subject left out)

#### Scenario: A header hidden in the address
- **WHEN** the member clicks `mailto:rita@example.com%0D%0Abcc:evil@example.org` or `mailto:rita@example.com%26attach=/etc/passwd`
- **THEN** nothing opens, while `mailto:rita@example.com,joao@example.com` opens in the mail app

#### Scenario: More addresses, or a field hidden in the body
- **WHEN** the member clicks `mailto:?to=rita@example.com%0D%0Abcc:evil@example.org`, or `mailto:rita@example.com?body=x%26attach%3D/etc/passwd`
- **THEN** the first opens nowhere, and the second opens with no body

#### Scenario: An authority or an invisible character
- **WHEN** the member clicks `mailto://x%0D%0Abcc:evil@example.org`, or an address holding U+202E, U+200B, U+0085 or a byte that is not UTF-8
- **THEN** nothing opens, while `mailto:ana@exämple.example` opens in the mail app

#### Scenario: A header hidden in the subject
- **WHEN** a mail link reads `mailto:rita@x.example?subject=Hi%0D%0ABcc:%20evil@x.example&body=Thanks`, or its body holds a NUL
- **THEN** the subject (or the body) is left out and the rest opens; a body over several lines, with tabs, is kept as written

#### Scenario: A field behind a semicolon or a second encoding
- **WHEN** a mail link reads `mailto:x@x.example?subject=x;attach=/etc/passwd`, or its subject is `Hi%250D%250ABcc:%20evil@x.example`
- **THEN** the subject is left out and the rest opens, while a subject of `50%25%20off` is kept as "50% off"

#### Scenario: The real shell
- **WHEN** CI's `tauri-shell` job opens the hostile mail in a debug build of the desktop app
- **THEN** the mail frame has no `__TAURI_INTERNALS__` or `__TAURI__`, nothing of the mail ran, its `target=_blank` link reaches the system opener and no new webview, and its meta refresh and nested iframe reach nothing

### Requirement: Only offers that take effect
The reader SHALL offer "Always show images from <sender>" and its ⇧A only
where the server says trusting the sender can take effect, with "Anyone at"
only for the domain the server offers; elsewhere, when remote images stay
held back, it SHALL say why in one line (under review, failing the sender's
checks, a provider without sender checks, or waiting for Scout's check). In
an inbox other members use, the note SHALL say the inbox trusts the sender.
The confirmation SHALL say that images wait for Scout's check, so they stay
held back while Scout is off or past its daily limit.

#### Scenario: Failing checks
- **WHEN** a message failing its sender's checks has its images held back
- **THEN** the reader says so in one line and offers no trust, and ⇧A does nothing

#### Scenario: A shared inbox
- **WHEN** a rule of a shared inbox trusts the Weekly Ledger
- **THEN** the note reads "this inbox trusts The Weekly Ledger"

### Requirement: Sending only what the member was shown
The composer SHALL send, with every save and with Send, the
`content_token` of the content on screen: the one of the draft it opened
with, then of each save's answer or re-read it applies, never one from a
list or a read it does not show. When an autosave answers 409
`content_changed` (or `draft_changed`), it SHALL keep what the member
typed, read the draft again for its version, token and attachments, show
the notice once, and save nothing on its own: the next edit or Send saves
the typed text with the fresh token. When Send answers 409
`content_changed` or `draft_changed` (saved since elsewhere: another tab,
another device, the assistant) it SHALL read the draft again and show it
as it is now (its recipients, Cc and Bcc included, its subject and text)
with its version and token, with "This changed since you opened it. Check
it and send again." in all four locales, and SHALL NOT send it again on its
own: the next Send names the fresh version and token. Text typed while the
refused Send was in flight SHALL be kept, taking only the fresh version and
token. The message composer SHALL show Cc
and Bcc whenever the draft has any, also ones that arrive after it opened.
The reply dialog SHALL show every recipient the reply goes to and SHALL
NOT reuse a draft it did not create (one from the drafts list, or the one
an Undo handed back): it SHALL create a fresh reply draft with exactly those
recipients, no Cc or Bcc, its own quoted text and no attachments, discard
the draft it replaces while that one is editable (a `draft`, or a `failed`
send) and never one that is `scheduled` or `sending`, which only its Undo
takes back, and send with the token of that draft; it MAY save
again only the draft it created, with its token. On 409 `content_changed`
or `draft_changed`, from that save or from Send, it SHALL say so and send
nothing until the member sends again, in a fresh draft. The forward dialog
SHALL save into an untouched forward draft with its version and token and
start a fresh forward when that is refused with either code.

#### Scenario: The draft changed since it was opened
- **WHEN** Sam presses Send and the server answers 409 `content_changed`
- **THEN** the composer shows the draft as the server holds it, a Bcc Sam never added included, with the notice, and nothing is sent until Sam presses Send again

#### Scenario: The draft was saved elsewhere since
- **WHEN** Sam has a draft open with nothing unsaved, adds a line to it on his phone, and presses Send here, and the server answers 409 `draft_changed`
- **THEN** the composer reads the draft again and shows it with the phone's line and the notice, nothing is sent, and the next Send carries the fresh version and token and goes

#### Scenario: An autosave refused
- **WHEN** the composer's autosave answers 409 `content_changed`
- **THEN** the typed text stays, the notice shows once, nothing is saved again until Sam types or presses Send, and that save uses the token of the draft read again

#### Scenario: A reply dialog after an Undo
- **WHEN** the draft an Undo handed back holds a Bcc and an attachment the dialog does not show, and Sam sends from the dialog
- **THEN** a fresh draft with only the recipients shown is created and sent, and the old one is discarded

#### Scenario: A reply dialog whose draft was saved elsewhere
- **WHEN** the reply draft the dialog created is saved elsewhere, and its save or Send answers 409 `draft_changed`
- **THEN** the dialog says the reply changed and sends nothing, and the next Send creates a fresh reply draft, sends it with its own version and token, and discards the old one

#### Scenario: Opened before the upgrade
- **WHEN** Sam types or presses Send in a composer opened before the upgrade
- **THEN** the notice shows once, Sam's text is still there, and the next save and Send work

#### Scenario: Replacing a draft whose send failed
- **WHEN** the draft an Undo or a failed send handed back is `failed`, and Sam opens Scout's text in the composer
- **THEN** a fresh reply draft is created and the failed one is discarded

#### Scenario: Replacing a draft on its way
- **WHEN** the draft handed back is `scheduled` (sent again elsewhere since)
- **THEN** a fresh reply draft is created and the scheduled one is left alone

### Requirement: Quoting mail whose words came later
When `conversations.get` marks a message `text_unread`, the client SHALL ask
for its words alone (`words=true` on its body), once per message, before
quoting it in a reply, reply all or forward (the composer, the reply dialog
and Home's Forward picker), and for its folded line in the reader. While
they are read, a neutral line SHALL say so. When they cannot be read,
nothing SHALL be started or sent and the member SHALL be told why: a reply
or forward SHALL never carry an empty quote silently, nor a quote the member
was not shown. In the reply dialog, Send, its shortcut and "Open in composer"
SHALL wait until the quote is shown, and "Try again" SHALL read the words
again and show them. When the words came `truncated`, the composer SHALL say
the quote holds the email's first part. A folded line whose words cannot be
read SHALL say so rather than stay blank.

#### Scenario: Forwarding a message whose words came later
- **WHEN** a member forwards a message marked `text_unread`
- **THEN** "Reading the email to quote it…" shows, and the forward's quote carries the message's words

#### Scenario: Its words cannot be read
- **WHEN** a member forwards a message marked `text_unread` and its words cannot be read
- **THEN** no draft is started and "Couldn't read the email to quote it" says why

#### Scenario: Replying after the quote could not be read
- **WHEN** the reply dialog could not read the words of the message it answers
- **THEN** Send is disabled and says why, and after "Try again" reads them the quote shows and Send may go

#### Scenario: A long email quoted
- **WHEN** a member forwards a message whose words came `truncated`
- **THEN** the composer says "This email is long: the quote holds its first part."

#### Scenario: A folded message whose words came later
- **WHEN** the reader folds a message marked `text_unread`
- **THEN** its line says "Reading…" until its words are read, then shows them

### Requirement: Saying what a quote leaves out
The composer SHALL say "the quote holds its first part" when the message it
quotes is `quote_cut` or its text or words came `truncated`. The reply
dialog's preview SHALL say when the reply quotes only the email's first part,
and otherwise, when the preview shows only the start of what the reply
quotes, that it is shown in part (the preview ending in "…").

#### Scenario: Replying to a long email
- **WHEN** a member opens Reply on a message marked `quote_cut`
- **THEN** the dialog's quote says "This email is long: your reply quotes its first part.", and Forward's composer says "This email is long: the quote holds its first part."

#### Scenario: A long quote in a short preview
- **WHEN** the reply dialog answers a message whose words are longer than its preview
- **THEN** the preview ends in "…" and says "Shown in part: your reply quotes all of it."

### Requirement: Delete and Report spam with undo
Delete (#) SHALL move the open conversation, or the one under the list's
cursor, to the trash, and Report spam (!) SHALL report it as spam, each shown
at once, with a toast ("Deleted · <subject>", "Reported as spam · <subject>")
and an entry that Undo (Z, or the toast) takes back, putting the conversation
back exactly where it was: Report spam on an archived conversation is undone
by Not spam and archiving it again, as one entry with one toast, while Not
spam on its own still brings received mail to the inbox. When it leaves the list
on screen, the next conversation opens (the list, on phones), as after Done.
The shortcuts SHALL be keyed by their operations (`conversations.trash`,
`conversations.report_spam`) and shown wherever their buttons are. Both SHALL
be icon buttons beside Pin and Mark read: one button style per toolbar.

#### Scenario: Delete and Undo
- **WHEN** the user presses # on an open conversation, then Z
- **THEN** it leaves the stream for Trash, the toast says "Deleted · <subject>", and Z restores it

#### Scenario: Report spam
- **WHEN** the user presses ! on a conversation in Primary
- **THEN** it leaves Primary for Spam, with Undo

#### Scenario: Undo Report spam from the archive
- **WHEN** the user presses ! on an archived conversation, then Z
- **THEN** it is out of Spam and back in Archive, not the inbox, in one Undo

### Requirement: The way back from each view
The reader's main action SHALL follow where the conversation is. In the inbox
it is Done (E). Archived (in Archive, or in Sent when archived) it SHALL be
Move to Inbox, on the same E. In Spam it SHALL be Not spam, on !, and in Trash
Restore, on #: the key that sent it there brings it back. Each SHALL have its
toast and Undo. In Spam and Trash the way back SHALL be the banner's button,
and E does nothing there. Spam SHALL offer no Delete, and # does nothing there:
moving it to the trash would clear its spam mark, and Not spam is its only way
out.

#### Scenario: No Delete in Spam
- **WHEN** the user opens a conversation in Spam and presses #
- **THEN** nothing happens; there is no Delete button, and ! (Not spam) is the way out

#### Scenario: Move to Inbox
- **WHEN** the user opens a conversation in Archive and presses E
- **THEN** it moves back to the inbox, the toast says "Moved to Inbox · <subject>", with Undo

#### Scenario: Restore
- **WHEN** the user opens a conversation in Trash and presses #
- **THEN** it goes back where it was, in the inbox or the archive, with Undo

#### Scenario: Not spam
- **WHEN** the user opens a conversation in Spam and presses !
- **THEN** its received mail is back in the inbox, with Undo

### Requirement: Sent rows
In Sent each row SHALL lead with whom the inbox's latest message there went
to ("To Marta Ferreira", "To Marta Ferreira and 2 others"), with that
person's avatar, and show the time of that message; it SHALL carry a
"Replied" mark when someone wrote back since. The list SHALL be ordered by
the inbox's latest message.

#### Scenario: They replied
- **WHEN** Maya answers a conversation the user wrote in last week
- **THEN** its Sent row stays where the user's message puts it and says "Replied"

### Requirement: Reading spam and the trash
A conversation in Spam SHALL say so in a plain banner ("This conversation is
in Spam.") with Not spam; one in Trash the same ("This conversation is in
Trash.") with Restore. In either, the reader SHALL offer no reply box, no
Reply, Reply all or Forward, and none of Scout's suggestions, needs or
drafts; restoring brings them back. In Spam remote images SHALL never load:
Show images, its ⇧I and "Always show images from …" are not offered, even
for a sender a rule trusts (the server says `images_held: spam`), and links
are inert as in mail that may be phishing.

#### Scenario: A trusted sender in Spam
- **WHEN** the user opens a reported newsletter from a sender a smart rule trusts
- **THEN** the banner says it is in Spam, its images stay held back with "Images never load in Spam.", and no Show images is offered

#### Scenario: No reply in the trash
- **WHEN** the user opens a conversation in Trash
- **THEN** there is no reply box and no Reply, and Restore brings both back

### Requirement: Who wrote last
A conversation row whose newest message the inbox sent SHALL start its
snippet with "You:" ("Eu:", "Yo:", "Moi :" in pt-PT, es-ES and fr), read
from the list's `latest_from_you`.

#### Scenario: Waiting on them
- **WHEN** Sam's reply is the newest message of a conversation in Primary
- **THEN** its row's snippet reads "You: Revised quote attached…"

### Requirement: An expired ask
A conversation row whose summary carries `lapsed_need` and no `need` SHALL
show the chip "Expired" in a neutral tone, and the reader's note SHALL show
the same chip, with no Done, Not needed, Yes or No for it. A still-needed
question SHALL be asked in the reader as on Home ("Pedro Alves asked 3 weeks
ago. Still to do?") with Yes (1) and No (2), under the "Check" chip.

#### Scenario: A sign-in link
- **WHEN** the user opens the inbox after a sign-in link's ten minutes have passed
- **THEN** its row shows "Expired" instead of "To do", and its reader offers nothing to resolve

#### Scenario: An old ask in the reader
- **WHEN** the user opens Pedro Alves's three-week-old request
- **THEN** the note asks "Pedro Alves asked 3 weeks ago. Still to do?" with Yes and No

### Requirement: Deleting a draft
While the composer is closed, the reader SHALL list every draft the member
can still edit on the conversation (state draft or failed), each as a chip
that names it ("Draft saved · Reply to <name>", "Draft saved · Reply all",
"Draft saved · Forward") and reopens it, with its own "Delete draft" beside
it. Deleting SHALL call `conversations.delete_draft`, remove the chip at once
and show a "Draft deleted" toast with Undo. Undo SHALL create the draft again
with the same kind, message, recipients, subject, text and quoted history; a
forward's attachments come back as a new forward carries them. The
composer's Discard SHALL do the same, restoring what was on screen, and
SHALL never save into the draft it deletes. When the server refuses a
delete, nothing SHALL change and the toast SHALL say so.

#### Scenario: Delete from the chip
- **WHEN** the member presses Esc on a reply draft, then Delete draft beside "Draft saved · Reply to Maya Okafor"
- **THEN** the draft is deleted, the chip goes, and the toast says "Draft deleted" with Undo

#### Scenario: Two drafts
- **WHEN** a conversation has a reply draft and a forward draft and the composer is closed
- **THEN** both are listed by name, and each can be deleted without touching the other

#### Scenario: Undo
- **WHEN** the member presses Undo on "Draft deleted"
- **THEN** a draft with the same recipients, subject, text and quoted history is back and listed

#### Scenario: Discard is undoable
- **WHEN** the member types in a draft, then chooses Discard
- **THEN** the draft is deleted, and Undo brings it back with the text that was typed

### Requirement: Drafts
Drafts (G then 0, `/inbox/drafts`) SHALL list the user's own saved drafts from
`conversations.list_drafts`, newest first, at most 50, without paging. Each
row SHALL be tagged "Draft" and lead with whom it is to (the first name, "+2"
for two more; "No recipients" when none), show the subject ("(no subject)" when empty), a line of its words and when it was
saved. Drafts already scheduled or sending SHALL not be listed. Opening a row
SHALL open the composer where the draft was left: a new message on its own
screen, an answer in its thread, which opens beside the Drafts list
(`/inbox/drafts/<conversation>?draft=<id>`, Back returning to Drafts) with the
composer on exactly that draft, whether it is a reply, a reply to all or a
forward. Each row SHALL
have a Delete draft icon button beside it, with a toast and Undo that writes the same draft again.
The list SHALL refresh when a draft is saved, sent or deleted. With none it
SHALL say "No drafts." in one line, and when exactly 50 are listed it SHALL
add "Your 50 newest drafts."

#### Scenario: Open a draft
- **WHEN** the user opens a saved new message in Drafts
- **THEN** the composer opens on that draft, with its recipients, subject and text

#### Scenario: Open a reply draft
- **WHEN** the user opens a saved reply in Drafts
- **THEN** its conversation opens beside the Drafts list with the reply composer on that draft

#### Scenario: Several drafts in one conversation
- **WHEN** a conversation holds a saved reply and a saved forward, and the user opens the forward's row in Drafts
- **THEN** the composer opens on the forward, not the reply

#### Scenario: Delete and Undo
- **WHEN** the user deletes a draft from the list, then presses Undo in the toast
- **THEN** it leaves the list with "Draft deleted", and Undo puts the same draft back

#### Scenario: Nothing saved
- **WHEN** the user has no drafts
- **THEN** Drafts says "No drafts." in one line

#### Scenario: The newest 50
- **WHEN** the user has 50 or more drafts
- **THEN** Drafts lists the newest 50 and notes "Your 50 newest drafts."

### Requirement: The active place's chip is in view on phones
When the place changes on a phone, the chip of the stream or view now open
SHALL scroll into view in the chip row, the nearest edge, without moving the
page sideways. The scroll SHALL animate, except under `prefers-reduced-motion`,
where it SHALL jump.

#### Scenario: Trash on a phone
- **WHEN** the user taps Trash, the last chip, at 375px
- **THEN** the chip row scrolls so the Trash chip is fully visible

#### Scenario: Reduced motion
- **WHEN** the user prefers reduced motion and opens Drafts
- **THEN** its chip is brought into view without animation

### Requirement: Rows leaving the list say what happened
When the member's own action takes a conversation out of the list on screen
(Done, Move to Inbox, Delete, Restore, Report spam, Not spam, or a move to
another stream), its row SHALL first say what happened, in place: an icon and
"Done", "Moved to Inbox", "Deleted", "Restored", "Reported as spam", "Not
spam" or "Moved to <stream>", followed by the subject, wiping in over the row.
Report spam and Delete SHALL use the danger colour, the others the success
colour. The row SHALL then fold away while the rows below close the gap, all
within `--duration-confirm`. A leaving row SHALL NOT be a button, SHALL be
skipped by the keyboard cursor, and SHALL be hidden from assistive tech (the
toast says the same). Rows leaving one after another SHALL keep their order.
Undo while a row folds SHALL put the conversation straight back. Under reduced
motion the row SHALL go at once. Rows that leave for any other reason (another
device, a sync) SHALL go at once.

#### Scenario: Done from the list
- **WHEN** the user presses E on the row under the cursor in Primary
- **THEN** the row says "Done" with the subject, folds away under a second, the rows below move up, and the cursor is on the next row

#### Scenario: Report spam in red
- **WHEN** the user presses ! on a row in Primary
- **THEN** the row says "Reported as spam" in the danger colour, then folds away

#### Scenario: Moved to another stream
- **WHEN** the user moves an open conversation from Primary to Newsletters
- **THEN** its row in Primary says "Moved to Newsletters", then folds away

#### Scenario: Undo while it folds
- **WHEN** the user presses # on a row, then Z before the row has gone
- **THEN** the conversation is back in the list as a normal row

### Requirement: Suggesting people in To, Cc and Bcc
In the composer (New message, Reply, Reply all and Forward's composer), To,
Cc and Bcc SHALL each suggest people as the member types: from two
characters, after typing settles for about 200 ms, the field SHALL ask
`conversations.list_correspondents` with what it holds and show up to 8 of
the people it answers in a list under the field, each with their avatar,
their name and their address (the address alone when there is no name).
People already in To, Cc or Bcc SHALL be left out. The field SHALL follow
the ARIA combobox pattern: focus stays in the input, the first person is
highlighted, ↑ and ↓ move the highlight (wrapping), Enter or Tab add the
highlighted person as a chip, a click or a tap adds the person clicked, and
Esc closes the list without closing the composer. Leaving the field and
adding a person SHALL close the list. While the list is closed, or holds
nobody, Enter, Tab, a comma and leaving the field SHALL add what was typed,
as before. The list SHALL show the keys that work in it, fit the field's
width at 375px and scroll when it is taller than the space it has.

#### Scenario: Picking with the keyboard
- **WHEN** Sam types "mar" in To and Marta Ferreira and Mário Lopes are suggested
- **THEN** Marta is highlighted; ↓ highlights Mário, Enter adds "Mário Lopes" as a chip and the list closes

#### Scenario: Already added
- **WHEN** Marta is already in To and Sam types "mar" in Cc
- **THEN** Cc suggests Mário and not Marta

#### Scenario: A new address
- **WHEN** Sam types "new@studio.example", Esc closes the list, and Sam presses Enter
- **THEN** new@studio.example is added as a chip and the composer stays open

#### Scenario: Esc twice
- **WHEN** the list is open and Sam presses Esc twice
- **THEN** the first Esc closes the list and the second closes the composer, keeping the draft

### Requirement: Ask Scout in the reader
The reader SHALL offer "Ask Scout" (the visor before it, `.` with its
hint) among the conversation's actions, opening the Scout panel for the
open conversation. Below `lg` it SHALL sit in the reader's overflow when
the actions don't fit. On a server where Scout can't chat it SHALL still
be offered, and the panel SHALL say why.

#### Scenario: Why it is here
- **WHEN** Sam opens a conversation in Updates and presses `.`
- **THEN** the Scout panel opens about it with "Why is this here?" to tap

### Requirement: Make it a task
The reader SHALL offer "Make it a task" with `n t` (the same "new task" key, here with the email as its source), which opens an inline form
(not a dialog) with the subject as the title, the due chips and Do/Pay. Add
SHALL call `tasks.create` with the conversation as the source, close the form,
and show "Added to your tasks" with Undo and Open.

#### Scenario: From the lease email
- **WHEN** Ana presses `n t` on "Lease for Rua das Flores 12", edits the title to "Send the signed lease to Ana", picks Fri and adds it
- **THEN** a task due Friday with that conversation as its source is created, and Tasks shows "From Ana Ribeiro · email" on it

### Requirement: The note asks who has the move
The reader's note ("Where things stand") for a conversation whose open need is
a question with the people of its email SHALL say the same as Home's card, with
the same three actions and "How Scout decided", in place of the Yes / No
question.

#### Scenario: Opening it
- **WHEN** the user opens the conversation of João's message to Ana Costa, copying them
- **THEN** the note reads "João Pereira wrote to Ana Costa. You're in Cc." with "Scout isn't sure" and the actions Nudge Ana…, Reply to João… and Nothing for me

### Requirement: The note asks whether a task from the thread is done
When an open task of the member that came from the conversation asks Done? and no
need is open, the "where things stand" note SHALL add "Your task “<title>” is
still open." and ask "Is it done?" with Done (1) and Not yet (2). While it asks,
it takes the place of a stream suggestion's choice.

#### Scenario: Ana opens the thread she replied in
- **WHEN** Ana opens the conversation "Send the signed lease" came from
- **THEN** the note says the task is still open and offers Done and Not yet

### Requirement: Room to write
The message composer (Reply, Reply all, Forward, New message, and a draft
reopened from Drafts) SHALL set From, To, Cc, Bcc and the subject at the
body text size, in rows of at least 40px. Its text area SHALL be at least
ten lines at the body size and grow with the text. Its footer, holding Send,
Delete draft and the save state, SHALL stay at the bottom of the composer
while the text scrolls.

#### Scenario: Long text in the reader
- **WHEN** the user writes 40 lines in a reply in the reader
- **THEN** the composer grows to 60% of the window, the text scrolls inside it, and Send stays in view

### Requirement: Formatting shows as formatting
The composer's text SHALL show bold, italic, strikethrough, links with
words, bulleted and numbered lists and quotes as formatting, never as the
marks it is saved as, with undo and redo on ⌘Z and ⇧⌘Z. It SHALL save to
and open from those marks (conversations, "The marks a message may carry"),
so a reopened draft looks as it was left. Pasted content SHALL keep its text
and only those kinds of formatting.

#### Scenario: Bold shows as bold
- **WHEN** the user selects "renew for a year" and presses ⌘B
- **THEN** the words show in bold, with no asterisks, and the draft saves `**renew for a year**`

#### Scenario: A draft reopens as it was left
- **WHEN** the user reopens a draft saved as two bullet lines, `- parking P2-14` and `- rent on the ~~5th~~ 1st`
- **THEN** the composer shows a bulleted list of two items, with "5th" struck through

#### Scenario: Typing an asterisk
- **WHEN** the user types `2 * 3 = 6`
- **THEN** it shows as typed, and the draft saves `2 \* 3 = 6`

#### Scenario: Pasting from a web page
- **WHEN** the user pastes a heading, an image and a bold sentence
- **THEN** the heading's text becomes a paragraph, the image is dropped, and the sentence stays bold

### Requirement: The formatting toolbar
The composer SHALL offer Bold (⌘B), Italic (⌘I), Strikethrough (⇧⌘X), Link
(⇧⌘K), Bullets (⇧⌘8), Numbers (⇧⌘7) and Quote (⇧⌘9), each with a tooltip
naming its key. The toolbar SHALL stay one row: in a composer narrower than
32rem, Strikethrough, Numbers and Quote SHALL fold into a More menu, and
their keys work either way.

#### Scenario: A phone
- **WHEN** the composer is open at 375px
- **THEN** its toolbar is one row with Bold, Italic, Link, Bullets and More, and nothing overflows sideways

### Requirement: Links in the composer
Link SHALL ask for the words and the address and SHALL take only `http:`,
`https:` and `mailto:` addresses. Pasting an address over selected words
SHALL link them.

#### Scenario: Link words to an address
- **WHEN** the user selects "the lease", presses ⇧⌘K and enters `https://example.com/4b`
- **THEN** "the lease" shows as a link, and the draft saves `[the lease](https://example.com/4b)`

#### Scenario: A link that is not allowed
- **WHEN** the user enters `javascript:alert(1)` as a link's address
- **THEN** no link is made, and the dialog says only web and mail addresses can be linked

### Requirement: Expanding the composer
From 48rem, Expand (⇧⌘E) SHALL open the same draft, with its cursor and undo
history, in a sheet about 820px wide over the page, and Shrink or Esc SHALL
put it back where it was. Below 48rem Expand SHALL be hidden.

#### Scenario: Expand and back
- **WHEN** the user presses ⇧⌘E while writing, types a sentence, and presses Esc
- **THEN** the sheet opens on the same text with the cursor where it was, the sentence is kept, and the composer is back in the reader

#### Scenario: No Expand on a phone
- **WHEN** the composer is open at 375px
- **THEN** Expand is not shown

### Requirement: Attaching files in the composer
The composer SHALL offer Attach (⇧⌘U), which opens the device's file picker
for several files. Files dragged onto the composer, or pasted into its text,
SHALL attach too; text pastes as before. Each file SHALL show "Uploading…"
until it is stored, then its name, size and a Remove button. Once anything
is attached, the composer SHALL show the total against 25 MB.

#### Scenario: Attach two files
- **WHEN** the user presses ⇧⌘U and picks two PDFs
- **THEN** both show as uploading, then with their names and sizes, and the total reads, for example, "1.2 MB of 25 MB"

#### Scenario: Drop a file
- **WHEN** the user drags a file over the composer
- **THEN** the composer shows "Drop to attach", and dropping the file attaches it

#### Scenario: Paste a screenshot
- **WHEN** the user pastes an image from the clipboard into the text
- **THEN** it is attached as a file and the text is unchanged

### Requirement: Files the composer refuses
Before uploading, the composer SHALL refuse a file that is empty, would take
the total past 25 MB, or has a blocked type. It SHALL say which file and
why in the composer, beside its files, and upload nothing for it. A refusal from the server SHALL
be said the same way.

#### Scenario: Too large
- **WHEN** 24 MB are attached and the user adds a 2 MB file
- **THEN** the composer says that file would take the message over 25 MB, and it is not attached

#### Scenario: A blocked type
- **WHEN** the user drops `setup.exe`
- **THEN** the composer says that file type can't be sent by email, and it is not attached

### Requirement: The note says when Scout could not read the message
When `conversations.get` says `need_read_failed`, the reader's note for an open
need SHALL show one muted line, "Scout couldn't read this one.", under the
need's sentence, in the user's language (English, European Portuguese,
Spanish from Spain and French). It SHALL show nothing for a read that is
pending, finished, or has a call left.

#### Scenario: A read that failed for good
- **WHEN** the user opens a conversation with an open need whose read failed on every call
- **THEN** the note shows the need's sentence and, under it, "Scout couldn't read this one."

#### Scenario: A read that did not fail
- **WHEN** the user opens a conversation with an open need whose read is done or pending
- **THEN** the note shows no such line

### Requirement: Save a thread to a page
With pages enabled, the reader SHALL offer "Save to page" in its More menu
and through a shortcut. It opens a dialog to pick an existing page (filtered
by name, pages already linking the thread first) or a new private page, and
what to add: a live thread block, a copy of the messages (saying who reads
the page, from its effective readers, and whether it is sent to Notion), or
Scout's summary (shown as Scout's card before anything is added; hidden when
Scout is unavailable). Attachments can be
included. Every save SHALL show an Undo toast.

#### Scenario: A live block in an existing page
- **WHEN** Ana saves "Kickoff agenda" to "Client onboarding" as a live thread block
- **THEN** the page ends with the thread block and a toast offers "Open page" and "Undo"

#### Scenario: Scout not configured
- **WHEN** the `complete` role is not configured and Ana opens "Save to page"
- **THEN** the dialog offers the live block and the copy, but not Scout's summary

#### Scenario: A copy to a shared page says who will read it
- **WHEN** Ana chooses a copy of the messages for "Acme", which Rui reads through a grant on the page above it
- **THEN** the dialog says Rui will read it before Ana saves

#### Scenario: Off while the server does not offer it
- **WHEN** mail and Scout in pages is off on the server
- **THEN** the reader has no "Save to page" entry and its shortcut does nothing

#### Scenario: An attachment over the cap
- **WHEN** Ana includes attachments and one is over 20 MiB
- **THEN** the dialog says which file is too large and nothing is added
