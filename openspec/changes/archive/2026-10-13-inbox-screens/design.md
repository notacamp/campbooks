## Context

The `conversations` crate already ingests, threads and sorts mail and exposes
list/get/body plus the tidy operations (#452); the change feed (#453)
invalidates the client's queries by record type. The client module is a
placeholder, the design system has the pieces (ConversationRow, MessageCard,
Note, ActionBar, Shell panes, Toast/Undo), and the approved prototype (shell
& inbox) fixes the layout: streams under Inbox in the sidebar, a list pane, a
reader with an action bar, the assistant's note with "Why?", the thread, and
the compose box at the end.

## Goals / Non-goals

Goals: a usable inbox at 375px and desktop, keyboard-first, with every
change undoable, untrusted HTML shown safely, and the decide pass's answers
visible as suggestions with their stored reasons. Non-goals are in the
proposal (sending, search, automatic sorting, image proxy).

## Decisions

### 1. Email HTML: sanitised on the server, shown in a sandbox

The server sanitises (`domain::sanitize`, ammonia 4.2.3): an allow-list of
tags and attributes, `<script>`/`<style>` dropped with their content, no
forms, frames, objects, `<base>`/`<meta>`, event handlers or comments; link
schemes `http`, `https`, `mailto`, `tel`, relative URLs dropped (mail has no
base); every link gets `target="_blank" rel="noopener noreferrer"`; `style`
attributes keep only declarations without `url(`, `image-set(`, `@import`,
`expression`, `behavior`, `-moz-binding` or a backslash escape. A second pass
over ammonia's canonical output applies the image policy: tracking pixels (≤
2×2, or hidden by style) are removed always; other remote images lose their
`src` (marked `data-blocked="remote"`, counted in `remote_images`) unless the
request says `show_images=true`.

The client renders the result only through `lib/ui/safe-html` (`SafeHtml`):
an `<iframe srcdoc>` with `sandbox="allow-popups
allow-popups-to-escape-sandbox"` (no scripts, opaque origin, links can only
open a new tab), `referrerpolicy=no-referrer` plus a `no-referrer` meta, and
its own CSP: `default-src 'none'; img-src data:` (plus `https: http:` once
the member chose "Show images"); `style-src 'unsafe-inline'; font-src data:;
form-action 'none'; base-uri 'none'`. Mail gets a white page in every theme.

A `srcdoc` frame inherits the page's CSP, so the page's policy must allow
what sanitised mail still uses: `style-src-attr 'unsafe-inline'` (inline
`style` attributes; style elements stay blocked) and `img-src https:` (only
reachable after "Show images", since the frame's policy is the narrower one).

Alternatives considered:
- **Client-only sanitising** (DOMPurify in the browser, raw HTML over the
  wire): a new JS dependency, raw tracking URLs in the browser's cache and
  memory, and every native client would need its own sanitiser. Rejected.
- **Server-only, rendered inline** (`dangerouslySetInnerHTML` of the
  sanitised HTML): one sanitiser bug from XSS in the app's origin, and
  email CSS would bleed into the app. Rejected (AGENTS.md forbids it).
- **A separate `text/html` body document** (`GET …/body.html` with its own
  `Content-Security-Policy: sandbox …`, in an `<iframe src>`): its policy
  would not inherit the page's, so the app CSP could stay as it was; but it
  needs a new HTML operation through the BFF (cookies on frame loads),
  a Tauri custom protocol, and it reverses the "never served as text/html"
  rule. Kept as the follow-up if the CSP relaxation is ever a problem.
- **CSP-only image blocking** (keep `src`, let the frame's policy block):
  simpler, but the tracking URLs would still reach the client and depend on
  every webview enforcing a meta CSP in `srcdoc`. Both layers are kept.
- **Image proxy** (fetch remote images server-side): hides the reader's IP
  too, but is new infrastructure for self-hosters. Later.

Copying the old app: the Rails app on `main` also sanitised server-side
(Loofah) and showed bodies in a sandboxed iframe. The split is kept on its
merits (defence in depth, no JS sanitiser, one sanitiser for every client),
not because it was there; nothing else is ported.

The frame has no scripts and no same-origin access, so its content height
cannot be measured from outside. It is sized from the markup (text length,
blocks, declared image heights, clamped to 96–1400 px) and scrolls inside
beyond that. Measuring would need `allow-same-origin` or a script in the
frame; both are ruled out.

### 2. List and reader: routes, two panes or two screens

`/w/$workspaceId/inbox/$stream` is the list; `…/$conversationId` adds the
reader. From 64rem both panes show side by side (the shell's panes); below,
`ListPane`/`ReaderPane` take `narrow="hidden"` so the list and the reader are
separate screens, with "Back to <stream>" (Esc). Below 48rem the streams
become chips and the reader's Done/Reply float above the tab bar
(`ActionBar phoneOnly aboveTabBar`).

Alternatives: one route with the open conversation in client state (no deep
links, the phone's back button leaves the inbox); the reader as an overlay
on phones (the prototype shows a full screen). Routes win: the URL is the
state and the change feed's invalidation needs nothing more.

### 3. Suggestions come from the server, with their reasons

`ConversationSummary.suggestion` is computed in the read (`domain::
suggestions::attach`): the newest stream answer on record for the
conversation (model before rules for the same message), only when it did not
act, names another stream, and no member placed the conversation.
`ConversationDetail.decisions` lists each question's answer about the newest
inbound message with `reasons` (`[{rule, evidence}]`) and `acted`. The
client translates rule ids (`reasons.signals.list_header` …) and shows
unknown ones as they are.

The reader's note says what the decide pass thinks as suggestions ("Campbooks'
read, as suggestions: it needs a reply and it mentions a date to meet"), the
placement line says who placed the conversation ("Suggests Newsletters, 95%
sure" / "Sorted into … by a rule" / "You put this in …" / "In Primary:
nothing moved it"), and "Why?" lists every stored reason, labelled by
question. A stream suggestion offers "Move to <stream>" (1). Nothing is
applied without the member.

Alternatives: the client fetching decisions per row (N+1, and the API would
expose every stored answer in lists); a separate `list_decisions` operation
(another round trip per conversation for what one subquery answers).

### 4. Stream counts: a read operation

`conversations.list_streams` (`GET /api/v1/w/{workspace_id}/streams`,
`conversations:read`, `?inbox=` optional) returns the four streams in order
with `conversations` and `unread` (conversations with an unread message) for
conversations still in the inbox. Records: `conversations.conversation`, so
the change feed refreshes the counts. Alternatives: counts in
`list_inboxes` (a different shape per inbox, and the sidebar wants
workspace-wide counts); counting in the client (wrong beyond the first page).

### 5. Shortcuts

Defaults reused: J/K, Enter, E (Done = archive), R (reply), Z (undo), Esc,
1 (take the suggestion). Module shortcuts (`modules/conversations/
shortcuts.ts`): S `conversations.star` (toggle), U
`conversations.mark_unread` (toggle read), M `conversations.move_to_stream`
(then 1–4 in the menu), A `item.reply_all`, F `item.forward`, Shift+I
`inbox.show_images`, G then 1–4 `inbox.stream.*`. Bare 1–4 for streams (as
the prototype sketches) would collide with `item.choice1/2`, which the note's
choice uses; the registry refuses the conflict, so streams take G then 1–4.
Every button shows its keys.

### 6. Reply slot (for the send slice, #101)

`components/Reader/ReplySlot.ts` defines `RenderReplySlot = (context:
ReplySlotContext) => ReactNode`. The context carries `workspaceId`,
`inboxId`, `conversation`, `messages` (oldest first) and `request: {mode:
"reply" | "reply_all" | "forward", messageId, nonce} | null`, plus
`onClose()`. `InboxScreen` takes `renderReplySlot` (and `composeAction` for a
"New message" button in the list header); the reader renders the slot at the
end of the thread and owns the Reply/Reply all/Forward buttons and R/A/F,
which set `request`; without a renderer those buttons are hidden. The reader
remounts per conversation (keyed), so the slot does too.

### 7. Fake Google with a demo mailbox

`scripts/fake-gmail.ts` gives every fake Google account a fictional Gmail
mailbox (threads, a newsletter with a tracking pixel, script, style sheet and
remote images, invoices with PDFs) served at `/gmail/v1/users/me/…` (profile,
labels, messages list/get, history), plus refresh-token grants and
`POST /fake/deliver` to drop one more message in, so recordings run on the
real sync path. Development only, as before.

## Data, events, scopes

All data stays in the `conversations` schema; the new reads are
read-only queries on `conversations`, `messages` and `decisions`. No new
tables, no migration, no new events. Scopes: `conversations:read` for
`list_streams`, `get`, `list` and `get_message_body`; the tidy operations
keep `conversations:write`. AI: no new call; the reader shows the stored
answers of the `decide` pass (rules always, the `decide` model when that role
is configured). Every member action (move, archive, star, read) has an undo.

## Risks

- The relaxed page CSP allows inline style attributes and https images in
  the app itself; scripts, styles sheets and connections stay same-origin.
- Estimated frame heights can leave a long message scrolling inside its
  frame.
- ammonia 4.2.3 is newer than the two-week rule: it fixes RUSTSEC-2021-0157
  and -0158, which `cargo deny` reports for 4.2.0.
