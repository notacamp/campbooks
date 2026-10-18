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
Unread.

#### Scenario: Open a stream from the keyboard
- **WHEN** the user presses G then 3
- **THEN** the Receipts stream opens

#### Scenario: A pinned conversation
- **WHEN** the user pins an older conversation in Primary
- **THEN** it moves to the top of Primary, marked pinned, and stays there as newer mail arrives

### Requirement: Suggestions shown as suggestions
The decide pass's answers SHALL be shown as suggestions, never applied by the
client on its own: a row whose conversation has a stream suggestion SHALL say
"Suggests <stream>"; the reader's note SHALL say what the decide pass thinks
in plain words, as suggestions, and who placed the conversation, with no
percentage. "How Campbooks decided" (closed by default) SHALL list, for each
question, the answer in words, the stored reasons in the user's language, the
source (rules or model), the confidence and the threshold the answer was held
to, and whether it acted. A stream suggestion SHALL offer "Move to <stream>"
(1), which the user can undo.

#### Scenario: Taking a suggestion
- **WHEN** the user opens a newsletter that the rules suggest for Newsletters and presses 1
- **THEN** it moves to Newsletters, the toast offers Undo, and "How Campbooks decided" had listed the mailing-list header reason

#### Scenario: No numbers up front
- **WHEN** the user opens a conversation whose stream answer has a confidence of 0.87
- **THEN** the note shows no percentage, and 0.87 appears only inside "How Campbooks decided"

### Requirement: Reading a conversation
The reader SHALL show the conversation's subject, the note, and its messages
newest first with the newest (and any unread) open and older ones folded to
one line until opened, each with sender, recipients, time and attachments.
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
The reader SHALL expose a slot at the end of the thread for the send slice's
composer, with Reply (R), Reply all (Shift+R) and Forward (F) that tell the slot
which message to answer and how; without a composer those actions SHALL be
hidden. Reply SHALL first open the reply dialog (Scout's likely replies and
draft); its "Write my own" and "Open in composer" SHALL open the slot's
composer on the newest message, the latter on the draft Scout wrote. Reply
all and Forward SHALL open the composer directly.

#### Scenario: No composer yet
- **WHEN** the inbox runs without a reply-slot renderer
- **THEN** the reader shows no Reply, Reply all or Forward

#### Scenario: Write my own
- **WHEN** the user presses R in the reader, then chooses "Write my own"
- **THEN** the dialog closes and the composer opens at the end of the thread, as before

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
with who wrote it and when, then the chips, then Done and Not needed (X)
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
mail provider), how many conversations it opens up, that it holds only for
mail passing the sender's checks and that tracking pixels stay removed, and
no offer to apply it to the mail already here. Saving SHALL show its result
and offer Undo. A message whose images a rule trusts SHALL say so ("Images
shown: you trust <sender>") instead of offering Show images. Removing the rule
in Scout's memory SHALL say, as it asks to confirm, that the images are held
back again; once removed, Show images is back. A member who only reads the inbox SHALL not be
offered it, nor SHALL anyone on mail that may be phishing. No visor: no AI
is involved.

#### Scenario: Trust the Ledger
- **WHEN** the member opens the Weekly Ledger's newsletter, presses Always show images from The Weekly Ledger, picks anyone at weekly-ledger.example and saves
- **THEN** a `show_images` rule for the domain is created, not applied to existing mail, and the reader shows the images with "Images shown: you trust The Weekly Ledger"

#### Scenario: Only reading
- **WHEN** the member's role in the inbox is reader
- **THEN** Show images is offered and Always show images is not

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
a line break, `;`, `?`, `=`, `/`, `&` or `#` encoded too, so no field hides in
a value), and only when it has no authority (`mailto://…`) and its address
part and its `to`, `cc` and `bcc` fields are nothing but addresses (none, or
comma-separated `local@domain`s, valid UTF-8, not starting with `/`, with no
control character, Unicode blank, invisible character such as a bidi
override, a zero-width space or a variation selector, or delimiter such as
CR, LF, `&`, `;`, `?` or `\`, even encoded; other Unicode, as in an
internationalised address, is kept); a subject or body holding `&` or `?`
once decoded SHALL be dropped, since a mail app that decodes before it splits
would read another field there, and so SHALL a subject that is not valid
UTF-8 or holds a control character (CR and LF included), a line or paragraph
separator (U+2028, U+2029) or an invisible character, and a body that is not
valid UTF-8 or holds a control character other than CR, LF and tab, since a
mail app that decodes first would start another header there; anything else nowhere; no remote URL SHALL be granted
IPC. CI SHALL check the frame in WebKit under the desktop and web policies
read from their definitions, with the app's own sandbox and referrer policy,
and a control proving the check would see a script run.

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
- **THEN** the mail app gets `mailto:rita@example.com?subject=hi`, or `mailto:rita@example.com?subject=x%3Battach%3D%2Fetc%2Fpasswd`

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
`content_changed` it SHALL read the draft again and show it as it is now
(its recipients, Cc and Bcc included, its subject and text), with "This
changed since you opened it. Check it and send again." in all four locales,
and SHALL NOT send it again on its own. The message composer SHALL show Cc
and Bcc whenever the draft has any, also ones that arrive after it opened.
The reply dialog SHALL show every recipient the reply goes to and SHALL
NOT reuse a draft it did not create (one from the drafts list, or the one
an Undo handed back): it SHALL create a fresh reply draft with exactly those
recipients, no Cc or Bcc, its own quoted text and no attachments, discard
the draft it replaces while that one is editable (a `draft`, or a `failed`
send) and never one that is `scheduled` or `sending`, which only its Undo
takes back, and send with the token of that draft; it MAY save
again only the draft it created, with its token. On 409 `content_changed`
it SHALL say so and send nothing until the member sends again, in a fresh
draft. The forward dialog SHALL save into an untouched forward draft with
its token and start a fresh forward when that is refused.

#### Scenario: The draft changed since it was opened
- **WHEN** Sam presses Send and the server answers 409 `content_changed`
- **THEN** the composer shows the draft as the server holds it, a Bcc Sam never added included, with the notice, and nothing is sent until Sam presses Send again

#### Scenario: An autosave refused
- **WHEN** the composer's autosave answers 409 `content_changed`
- **THEN** the typed text stays, the notice shows once, nothing is saved again until Sam types or presses Send, and that save uses the token of the draft read again

#### Scenario: A reply dialog after an Undo
- **WHEN** the draft an Undo handed back holds a Bcc and an attachment the dialog does not show, and Sam sends from the dialog
- **THEN** a fresh draft with only the recipients shown is created and sent, and the old one is discarded

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
