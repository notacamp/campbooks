# Design

Builds on part 1's server (`2026-10-28-pages-from-mail-and-scout`, its design
and "Deviations while building"). The prototype is the source of truth for
layout: the "Save a thread to a page" board and the Scout card on the "Pages"
board.

## When it shows

Everything here shows only while the server offers part 1's operations. The
client learns that the way it already learns that pages and tables are on
(the builder checks how; it must not be a new client-side flag). With the
flag off: no menu entry, no shortcut, no "Ask Scout" on a page, and
`RUNNABLE` still lists the page operations but no proposal can reach the
client.

## Save to page (reader)

- `ConversationReader.tsx`: a "Save to page" `OverflowAction` beside "Make it
  a task", with a shortcut in the shortcut catalogue (`pages.save_thread`;
  `⇧P` if free, else the next free key, shown as a hint). It opens
  `SaveToPageDialog`, exported from `~/modules/pages`'s `index.ts`.
- **Dialog** (one question per view, as the prototype):
  - "Add to a page" or "New page". New pages are private, at the top level.
  - The page picker filters by name. The pages that already link the thread
    come first (`pages.backlinks`), then recent ones. Tables are left out.
  - The three choices: a live thread block, a copy of the messages, and
    Scout's summary. Plus an "Include attachments (N)" checkbox when the
    thread has attachments. Save with `⌘↵`.
  - Choosing the copy shows who reads the target page (`pages.get_sharing`):
    "Only you will read it", "Rui and 2 others will read it" or "Everyone in
    the workspace will read it". For a synced page it adds "It's sent to
    Notion too".
  - "Scout's summary" turns Save into "Preview summary". That opens the Scout
    panel with a chat about the conversation that asks Scout for a section on
    that page. Scout's card is the preview, and nothing is added until the
    member chooses "Add to page". The choice is hidden when Scout is
    unavailable (the `complete` role is not configured).
- **The live block**: an `embed` of the conversation appended through
  `pages.sync`, as change 1's editor does.
- **The copy**: each message becomes a heading-3 line ("Sender — date")
  followed by its body. The body goes through the same sanitiser as paste,
  with the quoted history dropped as the reader drops it. It is appended in
  one sync.
- **Attachments**: `pages.attach_from_message`, then file blocks for the
  files it returns, in the same sync as the rest.
- **Undo toast** ("Saved to *page*", with "Open page" and "Undo"): the client
  removes the blocks it inserted (Yjs). Files it attached are detached with
  `pages.detach_files`. `attach_from_message`'s own refusals are each said
  by name, and nothing is half-added:
  - 403 `insufficient_scope`: this sign-in may not read mail;
  - 404: the thread changed, or a file is gone;
  - 413 `file_too_large`: a file is over 20 MiB.

  This is not the composer's draft upload, so its per-mailbox limits (#699)
  do not apply.

## The page change card (Scout)

- `PageChangeCard`, beside the other change cards, renders a proposal for
  `pages.append` or `pages.create`:
  - the target: the page title (or "New page under *parent*") with its
    breadcrumb;
  - who will read it, from the proposal's `audience`, `readers` and `others`:
    "Only you", "You, Rui and 2 others" or "Everyone in the workspace". Never
    "private" when anyone else can read it. Named readers are shown by the
    workspace's member names; ids alone are never shown;
  - "Sent to Notion too" when `notion` is true;
  - the full Markdown from `detail.markdown`, rendered with the page's own
    block styles (read-only TipTap or the page renderer). It is collapsed past
    about 12 lines with "Show all", never truncated without saying so;
  - source chips (threads) that open the source;
  - "Add to page", "Edit first" and "Discard". "Edit first" opens the
    Markdown in a plain editor inside the card; the edited text is sent.
    After adding, the card shows "Added", with "Open page" and "Undo"
    (`pages.unappend` or `pages.trash`).
- "Add to page" is enabled only once the card shows everything the member
  is confirming: the full Markdown (unfolded, or reachable through "Show
  all"), who will read it, and the Notion line when it applies. A proposal
  missing `detail.markdown` or the page's readers shows no "Add to page".
- `RUNNABLE` gains exactly `pages.append` and `pages.create`.

## Ask Scout on a page

- The page toolbar's More menu gets "Ask Scout" (it stays one row). It opens
  the Scout panel with a chat about the page (`SCOUT_CHAT_ABOUT` gains
  `pages.page`). It is hidden when Scout is unavailable.

## Copy, layout, quality

- en, pt-PT, es-ES and fr, with correct diacritics, through `pnpm test:fit`.
- Tokens only; toolbars stay one row; pickers search by name; every write
  has an Undo toast.
- Works at 375 px: the dialog becomes a sheet and the card wraps its actions.
- Stories for the dialog (each step), for the card (only you, named, everyone
  and Notion; long Markdown), and for "Ask Scout" on a page. Unit tests for
  the copy builder (headings, quoted history dropped, sanitised), the
  audience wording and the Undo path.

## Deviations while building

- **The flag is probed, not advertised.** The server does not say that mail and
  Scout in pages is on: `pages.list` carries only `tables`, and part 1 added no
  field. With the switch off every one of its routes is a 404, so the client
  asks `pages.get_sharing` for a page the caller can open (`useMailScoutAvailable`
  in `~/modules/pages`): the page on screen for "Ask Scout", the first page of
  the tree for the reader. A 200 means on, a 404 means off, anything else
  means not known yet (and nothing is offered). With no page to ask about, it
  is not known, so a workspace with no pages yet is not offered "Save to page"
  until it has one. A flag on `pages.list` (like `tables`) would make this one
  request instead of a probe, and is the follow-up if that matters.
- **"Scout unavailable" is the chat panel not being mounted.** The client has
  no read of the `complete` role: a screen learns that Scout is configured only
  when a chat starts (`chat_unavailable`). So "Scout's summary" and "Ask Scout"
  are hidden when no panel is mounted (`useScoutChat()` is `undefined`: stories,
  tests, a shell without it), and when the model is not configured the chat
  itself says so, as it does for every other "Ask Scout".
- **`RUNNABLE` gains exactly `pages.append` and `pages.create`.** Their undos
  (`pages.unappend`, `pages.trash`) are in a separate `PAGE_UNDOS` list, passed
  by `PageChangeCard` alone, so no other card can send them and `RUNNABLE`
  keeps its rule of listing what a card may confirm.
- **The summary hand-off closes the dialog, then opens Scout.** The opener has
  no "stay open" option, so Preview summary closes "Save to page" and opens the
  chat about the conversation with the question already asked (naming the page,
  or the new title). Scout's card is the preview.
- **The copy is text only.** Each message becomes a heading-3 "Sender — date"
  and paragraphs of its words, built as the page's own blocks (no HTML is read,
  so there is nothing to sanitise beyond control and bidirectional characters,
  which are removed). The quoted history is dropped by the reader, which hands
  the dialog each message's own words (`saveThreadOf`, with `splitQuoted`); a
  message whose words had not been read yet (`text_unread`) is read first.
- **Blocks are added to the Yjs document without the editor.** The dialog reads
  the page, appends the blocks to a copy of its document (the page's schema,
  through `prosemirrorJSONToYXmlFragment`) and sends one `pages.sync`. Undo
  reads the page again and removes the top-level blocks that still read the
  same (a block someone changed since stays), then detaches the files.
- **A shortcut in the `item` scope.** `pages.save_thread` is Shift P (free), with
  its hint; "Ask Scout" on a page uses the existing `scout.ask` key (`.`).
- **Not done: client events.** `thread_saved_to_page` and `page_proposal_settled`
  (named in part 1) are not emitted: settling a card is already tracked by the
  existing proposal event, and no patch 0024 was needed.
